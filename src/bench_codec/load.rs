//! `bytes -> Issue` for the specialized codec (benchmark only).
//!
//! One function per entity, one match arm per field. There is no encoder tree
//! and no dynamic dispatch: the JSON key is matched against byte-string
//! literals, the arm it selects knows the field's type statically, and the
//! parsed object goes straight into a stack slot. When the object ends, the
//! instance is allocated once and its `__slots__` are filled by offset.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDateTime, PyDelta, PyTzInfo};
use pyo3::{ffi, PyErr, PyResult};
use pyo3_ffi::Py_ssize_t;
use smallvec::SmallVec;

use super::obj::{Obj, Slot};
use super::scan::Scan;
use super::{alloc_instance, GithubIssueCodec, OrderedPlan, OrderedPlans, SKIP};

// --- error helpers ------------------------------------------------------

#[cold]
#[inline(never)]
fn missing(field: &'static str) -> PyErr {
    PyValueError::new_err(format!("missing required field {field:?}"))
}

#[cold]
#[inline(never)]
fn bad_enum(name: &'static str, value: &[u8]) -> PyErr {
    PyValueError::new_err(format!(
        "invalid {name} value {:?}",
        String::from_utf8_lossy(value)
    ))
}

#[cold]
#[inline(never)]
fn bad_datetime(value: &[u8]) -> PyErr {
    PyValueError::new_err(format!(
        "invalid datetime {:?}",
        String::from_utf8_lossy(value)
    ))
}

#[cold]
#[inline(never)]
fn bad_union(at: usize) -> PyErr {
    PyValueError::new_err(format!("expected object or string at position {at}"))
}

// --- scalar readers -----------------------------------------------------

#[inline(always)]
fn int_obj(py: Python<'_>, s: &mut Scan<'_>) -> PyResult<Obj> {
    let v = s.take_i64()?;
    let p = unsafe { ffi::PyLong_FromLongLong(v) };
    if p.is_null() {
        return Err(PyErr::fetch(py));
    }
    Ok(unsafe { Obj::from_owned(p) })
}

/// `null` -> `None`, anything else -> `$body`.
macro_rules! nullable {
    ($s:expr, $body:expr) => {
        if $s.cur() == b'n' {
            $s.take_null()?;
            Obj::none()
        } else {
            $body
        }
    };
}

/// JSON string -> enum member, via a byte matcher instead of a dict lookup.
macro_rules! enum_field {
    ($s:expr, $c:expr, $table:ident, $idx_of:ident, $name:literal) => {{
        let mut buf: SmallVec<[u8; 64]> = SmallVec::new();
        let text = $s.take_str_bytes(&mut buf)?;
        match $idx_of(text) {
            Some(i) => $c.$table.member(i),
            None => return Err(bad_enum($name, text)),
        }
    }};
}

#[inline]
fn datetime_obj(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut buf: SmallVec<[u8; 64]> = SmallVec::new();
    let text = s.take_str_bytes(&mut buf)?;
    parse_rfc3339(py, text, c)
}

#[inline(always)]
fn digit(b: u8) -> Option<u32> {
    let v = b.wrapping_sub(b'0');
    (v <= 9).then_some(v as u32)
}

#[inline(always)]
fn two(b: &[u8], i: usize) -> Option<u32> {
    Some(digit(b[i])? * 10 + digit(b[i + 1])?)
}

/// RFC 3339 `YYYY-MM-DD(T| )HH:MM:SS[.ffffff][Z|±HH:MM]`, straight into a
/// `datetime`. Replaces speedate + `PyDelta` + `PyTimeZone_FromOffset`: the
/// common `Z` case reuses the cached `timezone.utc` singleton.
fn parse_rfc3339(py: Python<'_>, b: &[u8], c: &GithubIssueCodec) -> PyResult<Obj> {
    let parsed = (|| {
        if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || b[13] != b':' || b[16] != b':' {
            return None;
        }
        if !matches!(b[10], b'T' | b't' | b' ') {
            return None;
        }
        let year = digit(b[0])? * 1000 + digit(b[1])? * 100 + digit(b[2])? * 10 + digit(b[3])?;
        let month = two(b, 5)?;
        let day = two(b, 8)?;
        let hour = two(b, 11)?;
        let minute = two(b, 14)?;
        let second = two(b, 17)?;

        let mut i = 19;
        let mut micro = 0u32;
        if b.get(i) == Some(&b'.') {
            i += 1;
            let frac_from = i;
            let mut scale = 100_000u32;
            while let Some(d) = b.get(i).copied().and_then(digit) {
                if scale > 0 {
                    micro += d * scale;
                    scale /= 10;
                }
                i += 1;
            }
            if i == frac_from {
                return None; // "12:00:00." with no digits
            }
        }

        let offset = match b.get(i) {
            None => None,
            Some(b'Z' | b'z') => {
                i += 1;
                Some(0i32)
            }
            Some(sign @ (b'+' | b'-')) => {
                let neg = *sign == b'-';
                i += 1;
                let oh = two(b, i)? as i32;
                i += 2;
                if b.get(i) == Some(&b':') {
                    i += 1;
                }
                let om = if b.len() > i { two(b, i)? as i32 } else { 0 };
                if b.len() > i {
                    i += 2;
                }
                let secs = oh * 3600 + om * 60;
                Some(if neg { -secs } else { secs })
            }
            Some(_) => return None,
        };
        if i != b.len() {
            return None;
        }
        Some((year, month, day, hour, minute, second, micro, offset))
    })();

    let Some((year, month, day, hour, minute, second, micro, offset)) = parsed else {
        return Err(bad_datetime(b));
    };

    let tz: Option<Bound<'_, PyTzInfo>> = match offset {
        None => None,
        Some(0) => Some(c.utc.bind(py).clone()),
        Some(secs) => Some(fixed_offset_tz(py, secs)?),
    };
    let dt = PyDateTime::new(
        py,
        year as i32,
        month as u8,
        day as u8,
        hour as u8,
        minute as u8,
        second as u8,
        micro,
        tz.as_ref(),
    )
    .map_err(|_| bad_datetime(b))?;
    Ok(unsafe { Obj::from_owned(dt.into_ptr()) })
}

#[cold]
fn fixed_offset_tz(py: Python<'_>, seconds: i32) -> PyResult<Bound<'_, PyTzInfo>> {
    let delta = PyDelta::new(py, 0, seconds, 0, true)?;
    let ptr = unsafe { pyo3_ffi::PyTimeZone_FromOffset(delta.as_ptr()) };
    if ptr.is_null() {
        return Err(PyErr::fetch(py));
    }
    Ok(unsafe { Bound::from_owned_ptr(py, ptr) }.cast_into()?)
}

// --- enum byte matchers -------------------------------------------------

#[inline(always)]
fn issue_state_idx(v: &[u8]) -> Option<usize> {
    match v {
        b"open" => Some(0),
        b"closed" => Some(1),
        _ => None,
    }
}

#[inline(always)]
fn milestone_state_idx(v: &[u8]) -> Option<usize> {
    match v {
        b"open" => Some(0),
        b"closed" => Some(1),
        _ => None,
    }
}

#[inline(always)]
fn state_reason_idx(v: &[u8]) -> Option<usize> {
    match v {
        b"completed" => Some(0),
        b"reopened" => Some(1),
        b"not_planned" => Some(2),
        _ => None,
    }
}

#[inline(always)]
fn author_association_idx(v: &[u8]) -> Option<usize> {
    match v {
        b"COLLABORATOR" => Some(0),
        b"CONTRIBUTOR" => Some(1),
        b"FIRST_TIMER" => Some(2),
        b"FIRST_TIME_CONTRIBUTOR" => Some(3),
        b"MANNEQUIN" => Some(4),
        b"MEMBER" => Some(5),
        b"NONE" => Some(6),
        b"OWNER" => Some(7),
        _ => None,
    }
}

// --- object driver ------------------------------------------------------

#[inline(always)]
fn slots<const N: usize>() -> [Slot; N] {
    std::array::from_fn(|_| Slot::empty())
}

/// Order-independent member loop: unknown keys are skipped, known ones dispatch
/// on the field index the key matcher returned.
macro_rules! read_object {
    ($py:ident, $s:ident, $c:ident, $f:ident, $key_of:ident, $field:ident) => {{
        $s.enter_object()?;
        let mut first = true;
        while let Some(key) = $s.next_key(first)? {
            first = false;
            let idx = $key_of(key);
            if idx == SKIP {
                $s.skip_value()?;
            } else {
                $field(idx, $py, $s, $c, &mut $f)?;
            }
        }
    }};
}

/// Oracle member loop: the plan says how long each key is and which field it
/// feeds, so the key text is stepped over without being read.
macro_rules! read_object_ordered {
    ($py:ident, $s:ident, $c:ident, $f:ident, $plan:expr, $field:ident) => {{
        let plan: &OrderedPlan = $plan;
        $s.enter_object()?;
        let mut first = true;
        for &(key_len, idx) in &plan.members {
            $s.skip_known_key(key_len, first)?;
            first = false;
            $field(idx, $py, $s, $c, &mut $f)?;
        }
        $s.ws();
        $s.expect(b'}', "ordered plan: expected '}'")?;
    }};
}

#[inline(always)]
fn req(slot: Slot, name: &'static str) -> PyResult<Obj> {
    slot.take().ok_or_else(|| missing(name))
}

#[inline(always)]
fn or_none(slot: Slot) -> Obj {
    slot.take().unwrap_or_else(Obj::none)
}

// --- User ---------------------------------------------------------------

pub(super) fn user_key(k: &[u8]) -> u8 {
    match k {
        b"login" => 0,
        b"id" => 1,
        b"node_id" => 2,
        b"avatar_url" => 3,
        b"gravatar_id" => 4,
        b"url" => 5,
        b"html_url" => 6,
        b"followers_url" => 7,
        b"following_url" => 8,
        b"gists_url" => 9,
        b"starred_url" => 10,
        b"subscriptions_url" => 11,
        b"organizations_url" => 12,
        b"repos_url" => 13,
        b"events_url" => 14,
        b"received_events_url" => 15,
        b"type" => 16,
        b"site_admin" => 17,
        b"name" => 18,
        b"email" => 19,
        b"starred_at" => 20,
        _ => SKIP,
    }
}

fn user_field(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    f: &mut [Slot; 21],
) -> PyResult<()> {
    match idx {
        0 => f[0].set(s.take_str(py)?),
        1 => f[1].set(int_obj(py, s)?),
        2 => f[2].set(s.take_str(py)?),
        3 => f[3].set(s.take_str(py)?),
        4 => f[4].set(nullable!(s, s.take_str(py)?)),
        5 => f[5].set(s.take_str(py)?),
        6 => f[6].set(s.take_str(py)?),
        7 => f[7].set(s.take_str(py)?),
        8 => f[8].set(s.take_str(py)?),
        9 => f[9].set(s.take_str(py)?),
        10 => f[10].set(s.take_str(py)?),
        11 => f[11].set(s.take_str(py)?),
        12 => f[12].set(s.take_str(py)?),
        13 => f[13].set(s.take_str(py)?),
        14 => f[14].set(s.take_str(py)?),
        15 => f[15].set(s.take_str(py)?),
        16 => f[16].set(s.take_str(py)?),
        17 => f[17].set(Obj::bool(s.take_bool()?)),
        18 => f[18].set(nullable!(s, s.take_str(py)?)),
        19 => f[19].set(nullable!(s, s.take_str(py)?)),
        20 => f[20].set(nullable!(s, datetime_obj(py, s, c)?)),
        _ => s.skip_value()?,
    }
    Ok(())
}

fn build_user(c: &GithubIssueCodec, f: [Slot; 21]) -> PyResult<Obj> {
    let l = &c.user;
    let o = alloc_instance(l.tp())?;
    let p = o.as_ptr();
    let [login, id, node_id, avatar_url, gravatar_id, url, html_url, followers_url, following_url, gists_url, starred_url, subscriptions_url, organizations_url, repos_url, events_url, received_events_url, type_, site_admin, name, email, starred_at] =
        f;
    unsafe {
        use super::slots::store_slot as st;
        st(p, l.login, req(login, "login")?);
        st(p, l.id, req(id, "id")?);
        st(p, l.node_id, req(node_id, "node_id")?);
        st(p, l.avatar_url, req(avatar_url, "avatar_url")?);
        st(p, l.gravatar_id, req(gravatar_id, "gravatar_id")?);
        st(p, l.url, req(url, "url")?);
        st(p, l.html_url, req(html_url, "html_url")?);
        st(p, l.followers_url, req(followers_url, "followers_url")?);
        st(p, l.following_url, req(following_url, "following_url")?);
        st(p, l.gists_url, req(gists_url, "gists_url")?);
        st(p, l.starred_url, req(starred_url, "starred_url")?);
        st(
            p,
            l.subscriptions_url,
            req(subscriptions_url, "subscriptions_url")?,
        );
        st(
            p,
            l.organizations_url,
            req(organizations_url, "organizations_url")?,
        );
        st(p, l.repos_url, req(repos_url, "repos_url")?);
        st(p, l.events_url, req(events_url, "events_url")?);
        st(
            p,
            l.received_events_url,
            req(received_events_url, "received_events_url")?,
        );
        st(p, l.type_, req(type_, "type")?);
        st(p, l.site_admin, req(site_admin, "site_admin")?);
        st(p, l.name, or_none(name));
        st(p, l.email, or_none(email));
        st(p, l.starred_at, or_none(starred_at));
    }
    Ok(o)
}

fn load_user(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<21>();
    read_object!(py, s, c, f, user_key, user_field);
    build_user(c, f)
}

fn load_user_ordered(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<21>();
    read_object_ordered!(py, s, c, f, plan(c, |p| &p.user)?, user_field);
    build_user(c, f)
}

// --- IssueLabel ---------------------------------------------------------

pub(super) fn label_key(k: &[u8]) -> u8 {
    match k {
        b"id" => 0,
        b"node_id" => 1,
        b"url" => 2,
        b"name" => 3,
        b"description" => 4,
        b"color" => 5,
        b"default" => 6,
        _ => SKIP,
    }
}

fn label_field(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    _c: &GithubIssueCodec,
    f: &mut [Slot; 7],
) -> PyResult<()> {
    match idx {
        0 => f[0].set(int_obj(py, s)?),
        1 => f[1].set(s.take_str(py)?),
        2 => f[2].set(s.take_str(py)?),
        3 => f[3].set(s.take_str(py)?),
        4 => f[4].set(nullable!(s, s.take_str(py)?)),
        5 => f[5].set(nullable!(s, s.take_str(py)?)),
        6 => f[6].set(Obj::bool(s.take_bool()?)),
        _ => s.skip_value()?,
    }
    Ok(())
}

fn build_label(c: &GithubIssueCodec, f: [Slot; 7]) -> PyResult<Obj> {
    let l = &c.label;
    let o = alloc_instance(l.tp())?;
    let p = o.as_ptr();
    let [id, node_id, url, name, description, color, default] = f;
    unsafe {
        use super::slots::store_slot as st;
        st(p, l.id, req(id, "id")?);
        st(p, l.node_id, req(node_id, "node_id")?);
        st(p, l.url, req(url, "url")?);
        st(p, l.name, req(name, "name")?);
        st(p, l.description, req(description, "description")?);
        st(p, l.color, req(color, "color")?);
        st(p, l.default, req(default, "default")?);
    }
    Ok(o)
}

fn load_label(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<7>();
    read_object!(py, s, c, f, label_key, label_field);
    build_label(c, f)
}

fn load_label_ordered(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<7>();
    read_object_ordered!(py, s, c, f, plan(c, |p| &p.label)?, label_field);
    build_label(c, f)
}

// --- Reactions ----------------------------------------------------------

pub(super) fn reactions_key(k: &[u8]) -> u8 {
    match k {
        b"url" => 0,
        b"total_count" => 1,
        b"+1" => 2,
        b"-1" => 3,
        b"laugh" => 4,
        b"confused" => 5,
        b"heart" => 6,
        b"hooray" => 7,
        b"eyes" => 8,
        b"rocket" => 9,
        _ => SKIP,
    }
}

fn reactions_field(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    _c: &GithubIssueCodec,
    f: &mut [Slot; 10],
) -> PyResult<()> {
    match idx {
        0 => f[0].set(s.take_str(py)?),
        1..=9 => f[idx as usize].set(int_obj(py, s)?),
        _ => s.skip_value()?,
    }
    Ok(())
}

fn build_reactions(c: &GithubIssueCodec, f: [Slot; 10]) -> PyResult<Obj> {
    let l = &c.reactions;
    let o = alloc_instance(l.tp())?;
    let p = o.as_ptr();
    let [url, total_count, plus_one, minus_one, laugh, confused, heart, hooray, eyes, rocket] = f;
    unsafe {
        use super::slots::store_slot as st;
        st(p, l.url, req(url, "url")?);
        st(p, l.total_count, req(total_count, "total_count")?);
        st(p, l.plus_one, req(plus_one, "+1")?);
        st(p, l.minus_one, req(minus_one, "-1")?);
        st(p, l.laugh, req(laugh, "laugh")?);
        st(p, l.confused, req(confused, "confused")?);
        st(p, l.heart, req(heart, "heart")?);
        st(p, l.hooray, req(hooray, "hooray")?);
        st(p, l.eyes, req(eyes, "eyes")?);
        st(p, l.rocket, req(rocket, "rocket")?);
    }
    Ok(o)
}

fn load_reactions(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<10>();
    read_object!(py, s, c, f, reactions_key, reactions_field);
    build_reactions(c, f)
}

fn load_reactions_ordered(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<10>();
    read_object_ordered!(py, s, c, f, plan(c, |p| &p.reactions)?, reactions_field);
    build_reactions(c, f)
}

// --- Milestone ----------------------------------------------------------

pub(super) fn milestone_key(k: &[u8]) -> u8 {
    match k {
        b"url" => 0,
        b"html_url" => 1,
        b"labels_url" => 2,
        b"id" => 3,
        b"node_id" => 4,
        b"number" => 5,
        b"title" => 6,
        b"description" => 7,
        b"creator" => 8,
        b"open_issues" => 9,
        b"closed_issues" => 10,
        b"created_at" => 11,
        b"updated_at" => 12,
        b"closed_at" => 13,
        b"due_on" => 14,
        b"state" => 15,
        _ => SKIP,
    }
}

fn milestone_field(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    f: &mut [Slot; 16],
) -> PyResult<()> {
    match idx {
        0 => f[0].set(s.take_str(py)?),
        1 => f[1].set(s.take_str(py)?),
        2 => f[2].set(s.take_str(py)?),
        3 => f[3].set(int_obj(py, s)?),
        4 => f[4].set(s.take_str(py)?),
        5 => f[5].set(int_obj(py, s)?),
        6 => f[6].set(s.take_str(py)?),
        7 => f[7].set(nullable!(s, s.take_str(py)?)),
        8 => f[8].set(nullable!(s, load_user(py, s, c)?)),
        9 => f[9].set(int_obj(py, s)?),
        10 => f[10].set(int_obj(py, s)?),
        11 => f[11].set(datetime_obj(py, s, c)?),
        12 => f[12].set(datetime_obj(py, s, c)?),
        13 => f[13].set(nullable!(s, datetime_obj(py, s, c)?)),
        14 => f[14].set(nullable!(s, datetime_obj(py, s, c)?)),
        15 => f[15].set(enum_field!(
            s,
            c,
            milestone_state,
            milestone_state_idx,
            "MilestoneState"
        )),
        _ => s.skip_value()?,
    }
    Ok(())
}

fn build_milestone(py: Python<'_>, c: &GithubIssueCodec, f: [Slot; 16]) -> PyResult<Obj> {
    let l = &c.milestone;
    let o = alloc_instance(l.tp())?;
    let p = o.as_ptr();
    let [url, html_url, labels_url, id, node_id, number, title, description, creator, open_issues, closed_issues, created_at, updated_at, closed_at, due_on, state] =
        f;
    unsafe {
        use super::slots::store_slot as st;
        st(p, l.url, req(url, "url")?);
        st(p, l.html_url, req(html_url, "html_url")?);
        st(p, l.labels_url, req(labels_url, "labels_url")?);
        st(p, l.id, req(id, "id")?);
        st(p, l.node_id, req(node_id, "node_id")?);
        st(p, l.number, req(number, "number")?);
        st(p, l.title, req(title, "title")?);
        st(p, l.description, req(description, "description")?);
        st(p, l.creator, req(creator, "creator")?);
        st(p, l.open_issues, req(open_issues, "open_issues")?);
        st(p, l.closed_issues, req(closed_issues, "closed_issues")?);
        st(p, l.created_at, req(created_at, "created_at")?);
        st(p, l.updated_at, req(updated_at, "updated_at")?);
        st(p, l.closed_at, req(closed_at, "closed_at")?);
        st(p, l.due_on, req(due_on, "due_on")?);
        st(
            p,
            l.state,
            state
                .take()
                .unwrap_or_else(|| Obj::new_ref(c.milestone_state_default.bind(py).as_ptr())),
        );
    }
    Ok(o)
}

fn load_milestone(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<16>();
    read_object!(py, s, c, f, milestone_key, milestone_field);
    build_milestone(py, c, f)
}

fn load_milestone_ordered(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<16>();
    read_object_ordered!(py, s, c, f, plan(c, |p| &p.milestone)?, milestone_field);
    build_milestone(py, c, f)
}

// --- lists --------------------------------------------------------------

#[inline]
fn build_list<A: smallvec::Array<Item = Obj>>(py: Python<'_>, items: SmallVec<A>) -> PyResult<Obj> {
    let lp = unsafe { ffi::PyList_New(items.len() as Py_ssize_t) };
    if lp.is_null() {
        return Err(PyErr::fetch(py));
    }
    for (i, item) in items.into_iter().enumerate() {
        unsafe { ffi::PyList_SET_ITEM(lp, i as Py_ssize_t, item.into_raw()) };
    }
    Ok(unsafe { Obj::from_owned(lp) })
}

/// `list[Union[IssueLabel, str]]`: the lead byte decides the member, matching the
/// real codec's "one viable member for this kind" narrowing.
fn load_labels(
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    ordered: bool,
) -> PyResult<Obj> {
    s.enter_array()?;
    let mut items: SmallVec<[Obj; 8]> = SmallVec::new();
    let mut first = true;
    while s.next_item(first)? {
        first = false;
        let item = match s.cur() {
            b'{' if ordered => load_label_ordered(py, s, c)?,
            b'{' => load_label(py, s, c)?,
            b'"' => s.take_str(py)?,
            _ => return Err(bad_union(s.i)),
        };
        items.push(item);
    }
    build_list(py, items)
}

fn load_users(
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    ordered: bool,
) -> PyResult<Obj> {
    s.enter_array()?;
    let mut items: SmallVec<[Obj; 8]> = SmallVec::new();
    let mut first = true;
    while s.next_item(first)? {
        first = false;
        items.push(if ordered {
            load_user_ordered(py, s, c)?
        } else {
            load_user(py, s, c)?
        });
    }
    build_list(py, items)
}

// --- Issue --------------------------------------------------------------

pub(super) fn issue_key(k: &[u8]) -> u8 {
    match k {
        b"id" => 0,
        b"node_id" => 1,
        b"url" => 2,
        b"repository_url" => 3,
        b"labels_url" => 4,
        b"comments_url" => 5,
        b"events_url" => 6,
        b"html_url" => 7,
        b"number" => 8,
        b"state" => 9,
        b"state_reason" => 10,
        b"title" => 11,
        b"body" => 12,
        b"user" => 13,
        b"labels" => 14,
        b"assignee" => 15,
        b"assignees" => 16,
        b"milestone" => 17,
        b"locked" => 18,
        b"active_lock_reason" => 19,
        b"comments" => 20,
        b"closed_at" => 21,
        b"created_at" => 22,
        b"updated_at" => 23,
        b"closed_by" => 24,
        b"author_association" => 25,
        b"draft" => 26,
        b"body_html" => 27,
        b"body_text" => 28,
        b"timeline_url" => 29,
        b"reactions" => 30,
        _ => SKIP,
    }
}

/// `ORDERED` is a const generic so the nested-entity calls monomorphize into two
/// separate code paths instead of branching per field.
fn issue_field<const ORDERED: bool>(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    f: &mut [Slot; 31],
) -> PyResult<()> {
    match idx {
        0 => f[0].set(int_obj(py, s)?),
        1 => f[1].set(s.take_str(py)?),
        2 => f[2].set(s.take_str(py)?),
        3 => f[3].set(s.take_str(py)?),
        4 => f[4].set(s.take_str(py)?),
        5 => f[5].set(s.take_str(py)?),
        6 => f[6].set(s.take_str(py)?),
        7 => f[7].set(s.take_str(py)?),
        8 => f[8].set(int_obj(py, s)?),
        9 => f[9].set(enum_field!(
            s,
            c,
            issue_state,
            issue_state_idx,
            "IssueState"
        )),
        10 => f[10].set(nullable!(
            s,
            enum_field!(s, c, state_reason, state_reason_idx, "IssueStateReason")
        )),
        11 => f[11].set(s.take_str(py)?),
        12 => f[12].set(nullable!(s, s.take_str(py)?)),
        13 => f[13].set(nullable!(
            s,
            if ORDERED {
                load_user_ordered(py, s, c)?
            } else {
                load_user(py, s, c)?
            }
        )),
        14 => f[14].set(load_labels(py, s, c, ORDERED)?),
        15 => f[15].set(nullable!(
            s,
            if ORDERED {
                load_user_ordered(py, s, c)?
            } else {
                load_user(py, s, c)?
            }
        )),
        16 => f[16].set(nullable!(s, load_users(py, s, c, ORDERED)?)),
        17 => f[17].set(nullable!(
            s,
            if ORDERED {
                load_milestone_ordered(py, s, c)?
            } else {
                load_milestone(py, s, c)?
            }
        )),
        18 => f[18].set(Obj::bool(s.take_bool()?)),
        19 => f[19].set(nullable!(s, s.take_str(py)?)),
        20 => f[20].set(int_obj(py, s)?),
        21 => f[21].set(nullable!(s, datetime_obj(py, s, c)?)),
        22 => f[22].set(datetime_obj(py, s, c)?),
        23 => f[23].set(datetime_obj(py, s, c)?),
        24 => f[24].set(nullable!(
            s,
            if ORDERED {
                load_user_ordered(py, s, c)?
            } else {
                load_user(py, s, c)?
            }
        )),
        25 => f[25].set(enum_field!(
            s,
            c,
            author_association,
            author_association_idx,
            "AuthorAssociation"
        )),
        26 => f[26].set(Obj::bool(s.take_bool()?)),
        27 => f[27].set(nullable!(s, s.take_str(py)?)),
        28 => f[28].set(nullable!(s, s.take_str(py)?)),
        29 => f[29].set(nullable!(s, s.take_str(py)?)),
        30 => f[30].set(nullable!(
            s,
            if ORDERED {
                load_reactions_ordered(py, s, c)?
            } else {
                load_reactions(py, s, c)?
            }
        )),
        _ => s.skip_value()?,
    }
    Ok(())
}

fn issue_field_plain(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    f: &mut [Slot; 31],
) -> PyResult<()> {
    issue_field::<false>(idx, py, s, c, f)
}

fn issue_field_ordered(
    idx: u8,
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
    f: &mut [Slot; 31],
) -> PyResult<()> {
    issue_field::<true>(idx, py, s, c, f)
}

fn build_issue(c: &GithubIssueCodec, f: [Slot; 31]) -> PyResult<Obj> {
    let l = &c.issue;
    let o = alloc_instance(l.tp())?;
    let p = o.as_ptr();
    let [id, node_id, url, repository_url, labels_url, comments_url, events_url, html_url, number, state, state_reason, title, body, user, labels, assignee, assignees, milestone, locked, active_lock_reason, comments, closed_at, created_at, updated_at, closed_by, author_association, draft, body_html, body_text, timeline_url, reactions] =
        f;
    unsafe {
        use super::slots::store_slot as st;
        st(p, l.id, req(id, "id")?);
        st(p, l.node_id, req(node_id, "node_id")?);
        st(p, l.url, req(url, "url")?);
        st(p, l.repository_url, req(repository_url, "repository_url")?);
        st(p, l.labels_url, req(labels_url, "labels_url")?);
        st(p, l.comments_url, req(comments_url, "comments_url")?);
        st(p, l.events_url, req(events_url, "events_url")?);
        st(p, l.html_url, req(html_url, "html_url")?);
        st(p, l.number, req(number, "number")?);
        st(p, l.state, req(state, "state")?);
        st(p, l.state_reason, req(state_reason, "state_reason")?);
        st(p, l.title, req(title, "title")?);
        st(p, l.body, req(body, "body")?);
        st(p, l.user, req(user, "user")?);
        st(p, l.labels, req(labels, "labels")?);
        st(p, l.assignee, req(assignee, "assignee")?);
        st(p, l.assignees, req(assignees, "assignees")?);
        st(p, l.milestone, req(milestone, "milestone")?);
        st(p, l.locked, req(locked, "locked")?);
        st(
            p,
            l.active_lock_reason,
            req(active_lock_reason, "active_lock_reason")?,
        );
        st(p, l.comments, req(comments, "comments")?);
        st(p, l.closed_at, req(closed_at, "closed_at")?);
        st(p, l.created_at, req(created_at, "created_at")?);
        st(p, l.updated_at, req(updated_at, "updated_at")?);
        st(p, l.closed_by, req(closed_by, "closed_by")?);
        st(
            p,
            l.author_association,
            req(author_association, "author_association")?,
        );
        st(p, l.draft, draft.take().unwrap_or_else(|| Obj::bool(false)));
        st(p, l.body_html, or_none(body_html));
        st(p, l.body_text, or_none(body_text));
        st(p, l.timeline_url, or_none(timeline_url));
        st(p, l.reactions, or_none(reactions));
    }
    Ok(o)
}

pub(super) fn load_issue(py: Python<'_>, s: &mut Scan<'_>, c: &GithubIssueCodec) -> PyResult<Obj> {
    let mut f = slots::<31>();
    read_object!(py, s, c, f, issue_key, issue_field_plain);
    build_issue(c, f)
}

pub(super) fn load_issue_ordered(
    py: Python<'_>,
    s: &mut Scan<'_>,
    c: &GithubIssueCodec,
) -> PyResult<Obj> {
    let mut f = slots::<31>();
    read_object_ordered!(py, s, c, f, plan(c, |p| &p.issue)?, issue_field_ordered);
    build_issue(c, f)
}

#[inline]
fn plan<'a>(
    c: &'a GithubIssueCodec,
    pick: impl Fn(&'a OrderedPlans) -> &'a Option<OrderedPlan>,
) -> PyResult<&'a OrderedPlan> {
    c.ordered
        .get()
        .and_then(|plans| pick(plans).as_ref())
        .ok_or_else(|| PyValueError::new_err("ordered plan missing for an entity"))
}
