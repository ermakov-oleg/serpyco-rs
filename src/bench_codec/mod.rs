//! Fully specialized JSON codec for `bench/compare/github_issue` (benchmark only).
//!
//! This is an experiment, not a feature: it answers "how low can the latency of
//! `bytes -> Issue -> bytes` go when the schema is known at build time and the
//! whole path is one piece of Rust?". It is compiled only under the
//! `bench-codec` cargo feature and is not reachable from the public
//! `serpyco_rs` API.
//!
//! What it deliberately does *not* do: no jiter, no serde_json, no JSON AST, no
//! Rust `Value`, no intermediate Python `dict`. `load` walks the input `&[u8]`
//! and writes finished `str`/`int`/`bool`/`datetime`/`list`/dataclass objects;
//! `dump` reads dataclass slots and appends bytes to one `Vec<u8>`.
//!
//! See `bench/experiments/specialized_json_codec/README.md` for the measured
//! numbers and the list of semantic differences from the real codec.

mod dump;
mod load;
mod obj;
mod scan;
mod simd;
mod slots;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyType, PyTzInfo};
use pyo3::{ffi, PyResult};
use pyo3_ffi::Py_ssize_t;

use obj::Obj;
use slots::{slot_offset, verify_layout};

/// Field index returned by the key matchers for a key the schema does not know.
pub(in crate::bench_codec) const SKIP: u8 = u8::MAX;

/// Declares the slot layout of one dataclass: a byte offset per field, resolved
/// once from the class object and verified against a probe instance.
macro_rules! layout {
    ($name:ident { $($field:ident : $py_name:literal),* $(,)? }) => {
        pub(in crate::bench_codec) struct $name {
            cls: Py<PyType>,
            $( pub(in crate::bench_codec) $field: Py_ssize_t, )*
        }

        impl $name {
            fn new(cls: &Bound<'_, PyType>) -> PyResult<Self> {
                $( let $field = slot_offset(cls, $py_name)?; )*
                verify_layout(cls, &[ $( ($py_name, $field) ),* ])?;
                Ok(Self { cls: cls.clone().unbind(), $( $field, )* })
            }

            #[inline(always)]
            pub(in crate::bench_codec) fn tp(&self) -> *mut ffi::PyTypeObject {
                self.cls.as_ptr() as *mut ffi::PyTypeObject
            }
        }
    };
}

layout!(IssueLayout {
    id: "id",
    node_id: "node_id",
    url: "url",
    repository_url: "repository_url",
    labels_url: "labels_url",
    comments_url: "comments_url",
    events_url: "events_url",
    html_url: "html_url",
    number: "number",
    state: "state",
    state_reason: "state_reason",
    title: "title",
    body: "body",
    user: "user",
    labels: "labels",
    assignee: "assignee",
    assignees: "assignees",
    milestone: "milestone",
    locked: "locked",
    active_lock_reason: "active_lock_reason",
    comments: "comments",
    closed_at: "closed_at",
    created_at: "created_at",
    updated_at: "updated_at",
    closed_by: "closed_by",
    author_association: "author_association",
    draft: "draft",
    body_html: "body_html",
    body_text: "body_text",
    timeline_url: "timeline_url",
    reactions: "reactions",
});

layout!(UserLayout {
    login: "login",
    id: "id",
    node_id: "node_id",
    avatar_url: "avatar_url",
    gravatar_id: "gravatar_id",
    url: "url",
    html_url: "html_url",
    followers_url: "followers_url",
    following_url: "following_url",
    gists_url: "gists_url",
    starred_url: "starred_url",
    subscriptions_url: "subscriptions_url",
    organizations_url: "organizations_url",
    repos_url: "repos_url",
    events_url: "events_url",
    received_events_url: "received_events_url",
    type_: "type",
    site_admin: "site_admin",
    name: "name",
    email: "email",
    starred_at: "starred_at",
});

layout!(IssueLabelLayout {
    id: "id",
    node_id: "node_id",
    url: "url",
    name: "name",
    description: "description",
    color: "color",
    default: "default",
});

layout!(MilestoneLayout {
    url: "url",
    html_url: "html_url",
    labels_url: "labels_url",
    id: "id",
    node_id: "node_id",
    number: "number",
    title: "title",
    description: "description",
    creator: "creator",
    open_issues: "open_issues",
    closed_issues: "closed_issues",
    created_at: "created_at",
    updated_at: "updated_at",
    closed_at: "closed_at",
    due_on: "due_on",
    state: "state",
});

layout!(ReactionsLayout {
    url: "url",
    total_count: "total_count",
    plus_one: "plus_one",
    minus_one: "minus_one",
    laugh: "laugh",
    confused: "confused",
    heart: "heart",
    hooray: "hooray",
    eyes: "eyes",
    rocket: "rocket",
});

/// One `Enum` subclass, resolved to members in declaration order.
///
/// Load side: the byte matchers in `load.rs` map the JSON string straight to an
/// index here — no dict lookup, no Python string. Dump side: member identity is
/// a pointer, so a linear scan over at most eight pointers picks the
/// pre-rendered `"value"` bytes.
pub(in crate::bench_codec) struct EnumTable {
    members: Vec<Py<PyAny>>,
    ptrs: Vec<usize>,
    encoded: Vec<Box<[u8]>>,
}

impl EnumTable {
    fn new(cls: &Bound<'_, PyType>, names: &[&str]) -> PyResult<Self> {
        let mut members = Vec::with_capacity(names.len());
        let mut ptrs = Vec::with_capacity(names.len());
        let mut encoded = Vec::with_capacity(names.len());
        for name in names {
            let member = cls.getattr(*name)?;
            let value: String = member.getattr("value")?.extract()?;
            let mut buf = Vec::with_capacity(value.len() + 2);
            buf.push(b'"');
            dump::escape_into(&mut buf, value.as_bytes());
            buf.push(b'"');
            ptrs.push(member.as_ptr() as usize);
            encoded.push(buf.into_boxed_slice());
            members.push(member.unbind());
        }
        Ok(EnumTable {
            members,
            ptrs,
            encoded,
        })
    }

    #[inline(always)]
    pub(in crate::bench_codec) fn member(&self, idx: usize) -> Obj {
        // Safety: `idx` comes from this table's own byte matcher.
        unsafe { Obj::new_ref(self.members[idx].as_ptr()) }
    }

    #[inline]
    pub(in crate::bench_codec) fn encoded_of(&self, p: *mut ffi::PyObject) -> Option<&[u8]> {
        let key = p as usize;
        self.ptrs
            .iter()
            .position(|&x| x == key)
            .map(|i| &*self.encoded[i])
    }
}

/// Member layout of one entity as it appears in the *priming* document: how many
/// bytes the key spans and which field it feeds. Only used by the oracle path.
pub(in crate::bench_codec) struct OrderedPlan {
    /// `(key length, field index)` in document order.
    pub(in crate::bench_codec) members: Vec<(usize, u8)>,
}

pub(in crate::bench_codec) struct OrderedPlans {
    pub(in crate::bench_codec) issue: Option<OrderedPlan>,
    pub(in crate::bench_codec) user: Option<OrderedPlan>,
    pub(in crate::bench_codec) label: Option<OrderedPlan>,
    pub(in crate::bench_codec) milestone: Option<OrderedPlan>,
    pub(in crate::bench_codec) reactions: Option<OrderedPlan>,
}

/// Specialized `bytes <-> Issue` codec.
///
/// Constructed from the benchmark's own classes so the prototype stays tied to
/// `bench/compare/github_issue/serpyco_rs.py` and produces exactly those
/// dataclasses.
#[pyclass(module = "serpyco_rs._serpyco_rs", frozen)]
pub struct GithubIssueCodec {
    pub(in crate::bench_codec) issue: IssueLayout,
    pub(in crate::bench_codec) user: UserLayout,
    pub(in crate::bench_codec) label: IssueLabelLayout,
    pub(in crate::bench_codec) milestone: MilestoneLayout,
    pub(in crate::bench_codec) reactions: ReactionsLayout,
    pub(in crate::bench_codec) issue_state: EnumTable,
    pub(in crate::bench_codec) milestone_state: EnumTable,
    pub(in crate::bench_codec) state_reason: EnumTable,
    pub(in crate::bench_codec) author_association: EnumTable,
    /// `datetime.timezone.utc`, the tzinfo every `Z`-suffixed timestamp gets.
    pub(in crate::bench_codec) utc: Py<PyTzInfo>,
    /// Identity of `utc` for the dump-side "is this UTC?" check.
    pub(in crate::bench_codec) utc_ptr: usize,
    /// `datetime.datetime` itself, for the dump-side type check.
    datetime_cls: Py<PyType>,
    /// `MilestoneState.OPEN`, the one non-`None` field default in the model.
    pub(in crate::bench_codec) milestone_state_default: Py<PyAny>,
    /// Size of the previous dump, used as the next buffer's capacity.
    dump_capacity: AtomicUsize,
    /// Set once by `enable_ordered`; the class stays `frozen`.
    pub(in crate::bench_codec) ordered: OnceLock<OrderedPlans>,
}

#[pymethods]
impl GithubIssueCodec {
    #[new]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        issue: &Bound<'_, PyType>,
        user: &Bound<'_, PyType>,
        label: &Bound<'_, PyType>,
        milestone: &Bound<'_, PyType>,
        reactions: &Bound<'_, PyType>,
        issue_state: &Bound<'_, PyType>,
        milestone_state: &Bound<'_, PyType>,
        state_reason: &Bound<'_, PyType>,
        author_association: &Bound<'_, PyType>,
    ) -> PyResult<Self> {
        let utc = py
            .import("datetime")?
            .getattr("timezone")?
            .getattr("utc")?
            .cast_into::<PyTzInfo>()?;
        let milestone_state_table = EnumTable::new(milestone_state, &["OPEN", "CLOSED"])?;
        let milestone_state_default = milestone_state_table.members[0].clone_ref(py);
        Ok(GithubIssueCodec {
            issue: IssueLayout::new(issue)?,
            user: UserLayout::new(user)?,
            label: IssueLabelLayout::new(label)?,
            milestone: MilestoneLayout::new(milestone)?,
            reactions: ReactionsLayout::new(reactions)?,
            issue_state: EnumTable::new(issue_state, &["OPEN", "CLOSED"])?,
            milestone_state: milestone_state_table,
            state_reason: EnumTable::new(state_reason, &["COMPLETED", "REOPENED", "NOT_PLANNED"])?,
            author_association: EnumTable::new(
                author_association,
                &[
                    "COLLABORATOR",
                    "CONTRIBUTOR",
                    "FIRST_TIMER",
                    "FIRST_TIME_CONTRIBUTOR",
                    "MANNEQUIN",
                    "MEMBER",
                    "NONE",
                    "OWNER",
                ],
            )?,
            utc_ptr: utc.as_ptr() as usize,
            utc: utc.unbind(),
            datetime_cls: py
                .import("datetime")?
                .getattr("datetime")?
                .cast_into::<PyType>()?
                .unbind(),
            milestone_state_default,
            dump_capacity: AtomicUsize::new(8192),
            ordered: OnceLock::new(),
        })
    }

    /// `bytes -> Issue`.
    fn load(&self, py: Python<'_>, data: &[u8]) -> PyResult<Py<PyAny>> {
        let mut s = scan::Scan::new(data);
        s.ws();
        let obj = load::load_issue(py, &mut s, self)?;
        s.finish()?;
        Ok(unsafe { Bound::from_owned_ptr(py, obj.into_raw()) }.unbind())
    }

    /// `Issue -> bytes`.
    fn dump<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let mut out = Vec::with_capacity(self.dump_capacity.load(Ordering::Relaxed));
        dump::dump_issue(py, self, value.as_ptr(), &mut out)?;
        self.dump_capacity.store(out.len(), Ordering::Relaxed);
        Ok(PyBytes::new(py, &out))
    }

    /// Walk the whole document with the same scanner but build nothing.
    ///
    /// Not a codec entry point — it exists so the benchmark can split "read and
    /// validate the bytes" from "materialize Python objects", the same split the
    /// jiter-only scan measures for the real codec.
    fn scan_only(&self, data: &[u8]) -> PyResult<usize> {
        let mut s = scan::Scan::new(data);
        s.ws();
        s.skip_value()?;
        s.finish()?;
        Ok(data.len())
    }

    /// Install the ordered-fixture oracle (see `load_ordered`).
    ///
    /// `plans` maps an entity name to the list of JSON keys as they appear in the
    /// priming document. Everything derived here — key spans, field order,
    /// which fields are absent — is knowledge about *this one document*.
    fn enable_ordered(&self, plans: &Bound<'_, PyDict>) -> PyResult<()> {
        let built = OrderedPlans {
            issue: take_plan(plans, "Issue", 31, load::issue_key)?,
            user: take_plan(plans, "User", 21, load::user_key)?,
            label: take_plan(plans, "IssueLabel", 7, load::label_key)?,
            milestone: take_plan(plans, "Milestone", 16, load::milestone_key)?,
            reactions: take_plan(plans, "Reactions", 10, load::reactions_key)?,
        };
        self.ordered
            .set(built)
            .map_err(|_| PyRuntimeError::new_err("enable_ordered() was already called"))
    }

    /// Oracle variant of [`load`]: assumes the exact member layout captured by
    /// `enable_ordered`, so no key text is read or compared at all — the cursor
    /// just steps over each key by its known length. Any document that deviates
    /// is rejected. This is *not* a general decoder; it exists to price the
    /// order-independent key dispatch.
    fn load_ordered(&self, py: Python<'_>, data: &[u8]) -> PyResult<Py<PyAny>> {
        let Some(plans) = self.ordered.get() else {
            return Err(PyRuntimeError::new_err("enable_ordered() was not called"));
        };
        if plans.issue.is_none() {
            return Err(PyRuntimeError::new_err("enable_ordered(): no Issue plan"));
        }
        let mut s = scan::Scan::new(data);
        s.ws();
        let obj = load::load_issue_ordered(py, &mut s, self)?;
        s.finish()?;
        Ok(unsafe { Bound::from_owned_ptr(py, obj.into_raw()) }.unbind())
    }
}

impl GithubIssueCodec {
    #[inline(always)]
    pub(in crate::bench_codec) fn datetime_type(&self) -> *mut ffi::PyTypeObject {
        self.datetime_cls.as_ptr() as *mut ffi::PyTypeObject
    }
}

fn take_plan(
    plans: &Bound<'_, PyDict>,
    name: &str,
    nfields: usize,
    matcher: fn(&[u8]) -> u8,
) -> PyResult<Option<OrderedPlan>> {
    let Some(keys) = plans.get_item(name)? else {
        return Ok(None);
    };
    let keys = keys.cast_into::<PyList>()?;
    let mut members = Vec::with_capacity(keys.len());
    let mut seen = vec![false; nfields];
    for key in keys.iter() {
        let key: String = key.extract()?;
        let idx = matcher(key.as_bytes());
        if idx == SKIP {
            return Err(PyValueError::new_err(format!(
                "{name}: ordered plan may not contain unknown key {key:?}"
            )));
        }
        if std::mem::replace(&mut seen[idx as usize], true) {
            return Err(PyValueError::new_err(format!(
                "{name}: ordered plan repeats key {key:?}"
            )));
        }
        members.push((key.len(), idx));
    }
    Ok(Some(OrderedPlan { members }))
}

/// `tp_alloc(cls, 0)`: the instance is zero-filled and GC-tracked, and every
/// slot is written before it escapes. `__init__`/`__post_init__` are skipped,
/// exactly as the real codec's `create_instance` does.
#[inline(always)]
pub(in crate::bench_codec) fn alloc_instance(tp: *mut ffi::PyTypeObject) -> PyResult<Obj> {
    let ptr = unsafe {
        let alloc = (*tp).tp_alloc.unwrap_or(ffi::PyType_GenericAlloc);
        alloc(tp, 0)
    };
    if ptr.is_null() {
        return Err(Python::attach(PyErr::fetch));
    }
    Ok(unsafe { Obj::from_owned(ptr) })
}
