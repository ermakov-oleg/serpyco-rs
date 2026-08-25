//! `Issue -> bytes` for the specialized codec (benchmark only).
//!
//! Field order, key text and separators are all fixed by the schema, so each
//! entity's dump is a straight-line sequence of "append the pre-rendered key
//! bytes, append the value". Values are read out of the dataclass `__slots__`
//! by offset — no `getattr`, no intermediate `dict`, no `PyUnicode` key objects.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::{ffi, PyErr, PyResult};
use pyo3_ffi::Py_ssize_t;

use super::simd::{find, Stop};
use super::slots::read_slot;
use super::{EnumTable, GithubIssueCodec};

// --- errors -------------------------------------------------------------

#[cold]
#[inline(never)]
fn unset(field: &'static str) -> PyErr {
    PyValueError::new_err(format!("field {field:?} is not set"))
}

#[cold]
#[inline(never)]
fn wrong_type(field: &'static str, expected: &'static str) -> PyErr {
    PyValueError::new_err(format!("field {field:?} is not a {expected}"))
}

// --- string escaping ----------------------------------------------------

#[cold]
#[inline(never)]
fn write_escaped_byte(buf: &mut Vec<u8>, b: u8) {
    match b {
        b'"' => buf.extend_from_slice(br#"\""#),
        b'\\' => buf.extend_from_slice(br"\\"),
        0x08 => buf.extend_from_slice(br"\b"),
        0x09 => buf.extend_from_slice(br"\t"),
        0x0A => buf.extend_from_slice(br"\n"),
        0x0C => buf.extend_from_slice(br"\f"),
        0x0D => buf.extend_from_slice(br"\r"),
        _ => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            buf.extend_from_slice(&[
                b'\\',
                b'u',
                b'0',
                b'0',
                HEX[(b >> 4) as usize],
                HEX[(b & 0x0F) as usize],
            ]);
        }
    }
}

/// Append `bytes` JSON-escaped, without the surrounding quotes: clean runs are
/// copied in bulk, and only the bytes JSON forbids are rewritten one at a time.
pub(super) fn escape_into(buf: &mut Vec<u8>, bytes: &[u8]) {
    let mut clean_from = 0;
    while let Some(at) = find(Stop::NeedsEscape, bytes, clean_from) {
        buf.extend_from_slice(&bytes[clean_from..at]);
        write_escaped_byte(buf, bytes[at]);
        clean_from = at + 1;
    }
    buf.extend_from_slice(&bytes[clean_from..]);
}

// --- scalar writers -----------------------------------------------------

/// Borrowed slot value; null (never assigned) is reported as an error.
///
/// # Safety
/// `p` must be an instance of the class `offset` was resolved from.
#[inline(always)]
unsafe fn slot(
    p: *mut ffi::PyObject,
    offset: Py_ssize_t,
    field: &'static str,
) -> PyResult<*mut ffi::PyObject> {
    let v = read_slot(p, offset);
    if v.is_null() {
        return Err(unset(field));
    }
    Ok(v)
}

/// UTF-8 bytes of a `str`. Compact-ASCII strings — every key and nearly every
/// value in this payload — are read straight out of the object.
#[inline(always)]
unsafe fn str_bytes<'a>(p: *mut ffi::PyObject) -> PyResult<&'a [u8]> {
    if ffi::PyUnicode_IS_COMPACT_ASCII(p) != 0 {
        let len = ffi::PyUnicode_GET_LENGTH(p) as usize;
        return Ok(std::slice::from_raw_parts(
            ffi::PyUnicode_1BYTE_DATA(p) as *const u8,
            len,
        ));
    }
    let mut size: Py_ssize_t = 0;
    let data = ffi::PyUnicode_AsUTF8AndSize(p, &mut size);
    if data.is_null() {
        return Err(Python::attach(PyErr::fetch));
    }
    Ok(std::slice::from_raw_parts(data as *const u8, size as usize))
}

#[inline(always)]
unsafe fn write_str(p: *mut ffi::PyObject, field: &'static str, out: &mut Vec<u8>) -> PyResult<()> {
    if ffi::PyUnicode_Check(p) == 0 {
        return Err(wrong_type(field, "str"));
    }
    let bytes = str_bytes(p)?;
    out.reserve(bytes.len() + 2);
    out.push(b'"');
    escape_into(out, bytes);
    out.push(b'"');
    Ok(())
}

#[inline(always)]
unsafe fn write_opt_str(
    p: *mut ffi::PyObject,
    field: &'static str,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if p == ffi::Py_None() {
        out.extend_from_slice(b"null");
        return Ok(());
    }
    write_str(p, field, out)
}

#[inline(always)]
unsafe fn write_int(p: *mut ffi::PyObject, field: &'static str, out: &mut Vec<u8>) -> PyResult<()> {
    if ffi::PyLong_Check(p) == 0 {
        return Err(wrong_type(field, "int"));
    }
    let v = ffi::PyLong_AsLongLong(p);
    if v == -1 && !ffi::PyErr_Occurred().is_null() {
        return Err(Python::attach(PyErr::fetch));
    }
    let mut b = itoa::Buffer::new();
    out.extend_from_slice(b.format(v).as_bytes());
    Ok(())
}

#[inline(always)]
unsafe fn write_bool(
    p: *mut ffi::PyObject,
    field: &'static str,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if p == ffi::Py_True() {
        out.extend_from_slice(b"true");
        Ok(())
    } else if p == ffi::Py_False() {
        out.extend_from_slice(b"false");
        Ok(())
    } else {
        Err(wrong_type(field, "bool"))
    }
}

#[inline(always)]
unsafe fn write_enum(
    p: *mut ffi::PyObject,
    table: &EnumTable,
    field: &'static str,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    match table.encoded_of(p) {
        Some(bytes) => {
            out.extend_from_slice(bytes);
            Ok(())
        }
        None => Err(wrong_type(field, "enum member")),
    }
}

#[inline(always)]
fn write_u32_padded(out: &mut Vec<u8>, mut v: u32, width: usize) {
    let mut buf = [b'0'; 6];
    let mut i = width;
    while i > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    out.extend_from_slice(&buf[..width]);
}

/// `YYYY-MM-DDTHH:MM:SS[.ffffff][Z|±HH:MM]`, byte-identical to what speedate's
/// `Display` produces for the values this model can hold.
unsafe fn write_datetime(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    field: &'static str,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if ffi::Py_TYPE(p) != c.datetime_type() {
        return Err(wrong_type(field, "datetime"));
    }
    out.reserve(40);
    out.push(b'"');
    write_u32_padded(out, ffi::PyDateTime_GET_YEAR(p) as u32, 4);
    out.push(b'-');
    write_u32_padded(out, ffi::PyDateTime_GET_MONTH(p) as u32, 2);
    out.push(b'-');
    write_u32_padded(out, ffi::PyDateTime_GET_DAY(p) as u32, 2);
    out.push(b'T');
    write_u32_padded(out, ffi::PyDateTime_DATE_GET_HOUR(p) as u32, 2);
    out.push(b':');
    write_u32_padded(out, ffi::PyDateTime_DATE_GET_MINUTE(p) as u32, 2);
    out.push(b':');
    write_u32_padded(out, ffi::PyDateTime_DATE_GET_SECOND(p) as u32, 2);
    let micro = ffi::PyDateTime_DATE_GET_MICROSECOND(p) as u32;
    if micro != 0 {
        out.push(b'.');
        write_u32_padded(out, micro, 6);
    }
    let tz = ffi::PyDateTime_DATE_GET_TZINFO(p);
    if tz as usize == c.utc_ptr {
        out.push(b'Z');
    } else if tz != ffi::Py_None() {
        write_tz_offset(py, p, tz, out)?;
    }
    out.push(b'"');
    Ok(())
}

/// A tzinfo other than the cached UTC singleton: ask Python for the offset.
#[cold]
unsafe fn write_tz_offset(
    py: Python<'_>,
    dt: *mut ffi::PyObject,
    tz: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    let tz = Bound::from_borrowed_ptr(py, tz);
    let dt = Bound::from_borrowed_ptr(py, dt);
    let offset = tz.call_method1("utcoffset", (dt,))?;
    if offset.is_none() {
        return Ok(());
    }
    let delta = offset.cast::<pyo3::types::PyDelta>()?;
    let secs = {
        use pyo3::types::PyDeltaAccess;
        delta.get_days() * 86400 + delta.get_seconds()
    };
    if secs == 0 {
        out.push(b'Z');
        return Ok(());
    }
    let total_minutes = secs / 60;
    out.push(if secs < 0 { b'-' } else { b'+' });
    write_u32_padded(out, (total_minutes / 60).unsigned_abs(), 2);
    out.push(b':');
    write_u32_padded(out, (total_minutes % 60).unsigned_abs(), 2);
    Ok(())
}

// --- entities -----------------------------------------------------------

unsafe fn dump_user(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if ffi::Py_TYPE(p) != c.user.tp() {
        return Err(wrong_type("user", "User"));
    }
    let l = &c.user;
    out.extend_from_slice(b"{\"login\":");
    write_str(slot(p, l.login, "login")?, "login", out)?;
    out.extend_from_slice(b",\"id\":");
    write_int(slot(p, l.id, "id")?, "id", out)?;
    out.extend_from_slice(b",\"node_id\":");
    write_str(slot(p, l.node_id, "node_id")?, "node_id", out)?;
    out.extend_from_slice(b",\"avatar_url\":");
    write_str(slot(p, l.avatar_url, "avatar_url")?, "avatar_url", out)?;
    out.extend_from_slice(b",\"gravatar_id\":");
    write_opt_str(slot(p, l.gravatar_id, "gravatar_id")?, "gravatar_id", out)?;
    out.extend_from_slice(b",\"url\":");
    write_str(slot(p, l.url, "url")?, "url", out)?;
    out.extend_from_slice(b",\"html_url\":");
    write_str(slot(p, l.html_url, "html_url")?, "html_url", out)?;
    out.extend_from_slice(b",\"followers_url\":");
    write_str(
        slot(p, l.followers_url, "followers_url")?,
        "followers_url",
        out,
    )?;
    out.extend_from_slice(b",\"following_url\":");
    write_str(
        slot(p, l.following_url, "following_url")?,
        "following_url",
        out,
    )?;
    out.extend_from_slice(b",\"gists_url\":");
    write_str(slot(p, l.gists_url, "gists_url")?, "gists_url", out)?;
    out.extend_from_slice(b",\"starred_url\":");
    write_str(slot(p, l.starred_url, "starred_url")?, "starred_url", out)?;
    out.extend_from_slice(b",\"subscriptions_url\":");
    write_str(
        slot(p, l.subscriptions_url, "subscriptions_url")?,
        "subscriptions_url",
        out,
    )?;
    out.extend_from_slice(b",\"organizations_url\":");
    write_str(
        slot(p, l.organizations_url, "organizations_url")?,
        "organizations_url",
        out,
    )?;
    out.extend_from_slice(b",\"repos_url\":");
    write_str(slot(p, l.repos_url, "repos_url")?, "repos_url", out)?;
    out.extend_from_slice(b",\"events_url\":");
    write_str(slot(p, l.events_url, "events_url")?, "events_url", out)?;
    out.extend_from_slice(b",\"received_events_url\":");
    write_str(
        slot(p, l.received_events_url, "received_events_url")?,
        "received_events_url",
        out,
    )?;
    out.extend_from_slice(b",\"type\":");
    write_str(slot(p, l.type_, "type")?, "type", out)?;
    out.extend_from_slice(b",\"site_admin\":");
    write_bool(slot(p, l.site_admin, "site_admin")?, "site_admin", out)?;
    out.extend_from_slice(b",\"name\":");
    write_opt_str(slot(p, l.name, "name")?, "name", out)?;
    out.extend_from_slice(b",\"email\":");
    write_opt_str(slot(p, l.email, "email")?, "email", out)?;
    out.extend_from_slice(b",\"starred_at\":");
    let starred_at = slot(p, l.starred_at, "starred_at")?;
    if starred_at == ffi::Py_None() {
        out.extend_from_slice(b"null");
    } else {
        write_datetime(py, c, starred_at, "starred_at", out)?;
    }
    out.push(b'}');
    Ok(())
}

unsafe fn dump_label(
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    let l = &c.label;
    out.extend_from_slice(b"{\"id\":");
    write_int(slot(p, l.id, "id")?, "id", out)?;
    out.extend_from_slice(b",\"node_id\":");
    write_str(slot(p, l.node_id, "node_id")?, "node_id", out)?;
    out.extend_from_slice(b",\"url\":");
    write_str(slot(p, l.url, "url")?, "url", out)?;
    out.extend_from_slice(b",\"name\":");
    write_str(slot(p, l.name, "name")?, "name", out)?;
    out.extend_from_slice(b",\"description\":");
    write_opt_str(slot(p, l.description, "description")?, "description", out)?;
    out.extend_from_slice(b",\"color\":");
    write_opt_str(slot(p, l.color, "color")?, "color", out)?;
    out.extend_from_slice(b",\"default\":");
    write_bool(slot(p, l.default, "default")?, "default", out)?;
    out.push(b'}');
    Ok(())
}

unsafe fn dump_reactions(
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if ffi::Py_TYPE(p) != c.reactions.tp() {
        return Err(wrong_type("reactions", "Reactions"));
    }
    let l = &c.reactions;
    out.extend_from_slice(b"{\"url\":");
    write_str(slot(p, l.url, "url")?, "url", out)?;
    out.extend_from_slice(b",\"total_count\":");
    write_int(slot(p, l.total_count, "total_count")?, "total_count", out)?;
    out.extend_from_slice(b",\"+1\":");
    write_int(slot(p, l.plus_one, "+1")?, "+1", out)?;
    out.extend_from_slice(b",\"-1\":");
    write_int(slot(p, l.minus_one, "-1")?, "-1", out)?;
    out.extend_from_slice(b",\"laugh\":");
    write_int(slot(p, l.laugh, "laugh")?, "laugh", out)?;
    out.extend_from_slice(b",\"confused\":");
    write_int(slot(p, l.confused, "confused")?, "confused", out)?;
    out.extend_from_slice(b",\"heart\":");
    write_int(slot(p, l.heart, "heart")?, "heart", out)?;
    out.extend_from_slice(b",\"hooray\":");
    write_int(slot(p, l.hooray, "hooray")?, "hooray", out)?;
    out.extend_from_slice(b",\"eyes\":");
    write_int(slot(p, l.eyes, "eyes")?, "eyes", out)?;
    out.extend_from_slice(b",\"rocket\":");
    write_int(slot(p, l.rocket, "rocket")?, "rocket", out)?;
    out.push(b'}');
    Ok(())
}

unsafe fn dump_milestone(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if ffi::Py_TYPE(p) != c.milestone.tp() {
        return Err(wrong_type("milestone", "Milestone"));
    }
    let l = &c.milestone;
    out.extend_from_slice(b"{\"url\":");
    write_str(slot(p, l.url, "url")?, "url", out)?;
    out.extend_from_slice(b",\"html_url\":");
    write_str(slot(p, l.html_url, "html_url")?, "html_url", out)?;
    out.extend_from_slice(b",\"labels_url\":");
    write_str(slot(p, l.labels_url, "labels_url")?, "labels_url", out)?;
    out.extend_from_slice(b",\"id\":");
    write_int(slot(p, l.id, "id")?, "id", out)?;
    out.extend_from_slice(b",\"node_id\":");
    write_str(slot(p, l.node_id, "node_id")?, "node_id", out)?;
    out.extend_from_slice(b",\"number\":");
    write_int(slot(p, l.number, "number")?, "number", out)?;
    out.extend_from_slice(b",\"title\":");
    write_str(slot(p, l.title, "title")?, "title", out)?;
    out.extend_from_slice(b",\"description\":");
    write_opt_str(slot(p, l.description, "description")?, "description", out)?;
    out.extend_from_slice(b",\"creator\":");
    let creator = slot(p, l.creator, "creator")?;
    if creator == ffi::Py_None() {
        out.extend_from_slice(b"null");
    } else {
        dump_user(py, c, creator, out)?;
    }
    out.extend_from_slice(b",\"open_issues\":");
    write_int(slot(p, l.open_issues, "open_issues")?, "open_issues", out)?;
    out.extend_from_slice(b",\"closed_issues\":");
    write_int(
        slot(p, l.closed_issues, "closed_issues")?,
        "closed_issues",
        out,
    )?;
    out.extend_from_slice(b",\"created_at\":");
    write_datetime(
        py,
        c,
        slot(p, l.created_at, "created_at")?,
        "created_at",
        out,
    )?;
    out.extend_from_slice(b",\"updated_at\":");
    write_datetime(
        py,
        c,
        slot(p, l.updated_at, "updated_at")?,
        "updated_at",
        out,
    )?;
    out.extend_from_slice(b",\"closed_at\":");
    write_opt_datetime(py, c, slot(p, l.closed_at, "closed_at")?, "closed_at", out)?;
    out.extend_from_slice(b",\"due_on\":");
    write_opt_datetime(py, c, slot(p, l.due_on, "due_on")?, "due_on", out)?;
    out.extend_from_slice(b",\"state\":");
    write_enum(slot(p, l.state, "state")?, &c.milestone_state, "state", out)?;
    out.push(b'}');
    Ok(())
}

#[inline(always)]
unsafe fn write_opt_datetime(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    field: &'static str,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if p == ffi::Py_None() {
        out.extend_from_slice(b"null");
        Ok(())
    } else {
        write_datetime(py, c, p, field, out)
    }
}

#[inline(always)]
unsafe fn write_opt_user(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if p == ffi::Py_None() {
        out.extend_from_slice(b"null");
        Ok(())
    } else {
        dump_user(py, c, p, out)
    }
}

pub(super) fn dump_issue(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    unsafe {
        if ffi::Py_TYPE(p) != c.issue.tp() {
            return Err(wrong_type("<root>", "Issue"));
        }
        let l = &c.issue;
        out.extend_from_slice(b"{\"id\":");
        write_int(slot(p, l.id, "id")?, "id", out)?;
        out.extend_from_slice(b",\"node_id\":");
        write_str(slot(p, l.node_id, "node_id")?, "node_id", out)?;
        out.extend_from_slice(b",\"url\":");
        write_str(slot(p, l.url, "url")?, "url", out)?;
        out.extend_from_slice(b",\"repository_url\":");
        write_str(
            slot(p, l.repository_url, "repository_url")?,
            "repository_url",
            out,
        )?;
        out.extend_from_slice(b",\"labels_url\":");
        write_str(slot(p, l.labels_url, "labels_url")?, "labels_url", out)?;
        out.extend_from_slice(b",\"comments_url\":");
        write_str(
            slot(p, l.comments_url, "comments_url")?,
            "comments_url",
            out,
        )?;
        out.extend_from_slice(b",\"events_url\":");
        write_str(slot(p, l.events_url, "events_url")?, "events_url", out)?;
        out.extend_from_slice(b",\"html_url\":");
        write_str(slot(p, l.html_url, "html_url")?, "html_url", out)?;
        out.extend_from_slice(b",\"number\":");
        write_int(slot(p, l.number, "number")?, "number", out)?;
        out.extend_from_slice(b",\"state\":");
        write_enum(slot(p, l.state, "state")?, &c.issue_state, "state", out)?;
        out.extend_from_slice(b",\"state_reason\":");
        let state_reason = slot(p, l.state_reason, "state_reason")?;
        if state_reason == ffi::Py_None() {
            out.extend_from_slice(b"null");
        } else {
            write_enum(state_reason, &c.state_reason, "state_reason", out)?;
        }
        out.extend_from_slice(b",\"title\":");
        write_str(slot(p, l.title, "title")?, "title", out)?;
        out.extend_from_slice(b",\"body\":");
        write_opt_str(slot(p, l.body, "body")?, "body", out)?;
        out.extend_from_slice(b",\"user\":");
        write_opt_user(py, c, slot(p, l.user, "user")?, out)?;
        out.extend_from_slice(b",\"labels\":");
        dump_labels(py, c, slot(p, l.labels, "labels")?, out)?;
        out.extend_from_slice(b",\"assignee\":");
        write_opt_user(py, c, slot(p, l.assignee, "assignee")?, out)?;
        out.extend_from_slice(b",\"assignees\":");
        let assignees = slot(p, l.assignees, "assignees")?;
        if assignees == ffi::Py_None() {
            out.extend_from_slice(b"null");
        } else {
            dump_users(py, c, assignees, out)?;
        }
        out.extend_from_slice(b",\"milestone\":");
        let milestone = slot(p, l.milestone, "milestone")?;
        if milestone == ffi::Py_None() {
            out.extend_from_slice(b"null");
        } else {
            dump_milestone(py, c, milestone, out)?;
        }
        out.extend_from_slice(b",\"locked\":");
        write_bool(slot(p, l.locked, "locked")?, "locked", out)?;
        out.extend_from_slice(b",\"active_lock_reason\":");
        write_opt_str(
            slot(p, l.active_lock_reason, "active_lock_reason")?,
            "active_lock_reason",
            out,
        )?;
        out.extend_from_slice(b",\"comments\":");
        write_int(slot(p, l.comments, "comments")?, "comments", out)?;
        out.extend_from_slice(b",\"closed_at\":");
        write_opt_datetime(py, c, slot(p, l.closed_at, "closed_at")?, "closed_at", out)?;
        out.extend_from_slice(b",\"created_at\":");
        write_datetime(
            py,
            c,
            slot(p, l.created_at, "created_at")?,
            "created_at",
            out,
        )?;
        out.extend_from_slice(b",\"updated_at\":");
        write_datetime(
            py,
            c,
            slot(p, l.updated_at, "updated_at")?,
            "updated_at",
            out,
        )?;
        out.extend_from_slice(b",\"closed_by\":");
        write_opt_user(py, c, slot(p, l.closed_by, "closed_by")?, out)?;
        out.extend_from_slice(b",\"author_association\":");
        write_enum(
            slot(p, l.author_association, "author_association")?,
            &c.author_association,
            "author_association",
            out,
        )?;
        out.extend_from_slice(b",\"draft\":");
        write_bool(slot(p, l.draft, "draft")?, "draft", out)?;
        out.extend_from_slice(b",\"body_html\":");
        write_opt_str(slot(p, l.body_html, "body_html")?, "body_html", out)?;
        out.extend_from_slice(b",\"body_text\":");
        write_opt_str(slot(p, l.body_text, "body_text")?, "body_text", out)?;
        out.extend_from_slice(b",\"timeline_url\":");
        write_opt_str(
            slot(p, l.timeline_url, "timeline_url")?,
            "timeline_url",
            out,
        )?;
        out.extend_from_slice(b",\"reactions\":");
        let reactions = slot(p, l.reactions, "reactions")?;
        if reactions == ffi::Py_None() {
            out.extend_from_slice(b"null");
        } else {
            dump_reactions(c, reactions, out)?;
        }
        out.push(b'}');
        Ok(())
    }
}

unsafe fn dump_labels(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if ffi::PyList_Check(p) == 0 {
        return Err(wrong_type("labels", "list"));
    }
    out.push(b'[');
    let n = ffi::PyList_GET_SIZE(p);
    for i in 0..n {
        if i != 0 {
            out.push(b',');
        }
        let item = ffi::PyList_GET_ITEM(p, i);
        if ffi::PyUnicode_Check(item) != 0 {
            write_str(item, "labels", out)?;
        } else if ffi::Py_TYPE(item) == c.label.tp() {
            dump_label(c, item, out)?;
        } else {
            return Err(wrong_type("labels", "IssueLabel | str"));
        }
    }
    out.push(b']');
    let _ = py;
    Ok(())
}

unsafe fn dump_users(
    py: Python<'_>,
    c: &GithubIssueCodec,
    p: *mut ffi::PyObject,
    out: &mut Vec<u8>,
) -> PyResult<()> {
    if ffi::PyList_Check(p) == 0 {
        return Err(wrong_type("assignees", "list"));
    }
    out.push(b'[');
    let n = ffi::PyList_GET_SIZE(p);
    for i in 0..n {
        if i != 0 {
            out.push(b',');
        }
        dump_user(py, c, ffi::PyList_GET_ITEM(p, i), out)?;
    }
    out.push(b']');
    Ok(())
}
