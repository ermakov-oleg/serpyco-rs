"""Hand-written Python builder: `dict -> Issue`, with no parsing at all.

A reference point for the experiment, not a contender: it starts from an
already-parsed `dict` (so the JSON scan, the string allocations and the integer
allocations are all somebody else's cost) and does nothing but walk the schema
by hand. It is the cheapest way anyone could write this conversion in Python, so
it bounds what a Python-level "compiled" loader could ever reach.
"""

from datetime import datetime
from typing import Any, Optional

from bench.compare.github_issue.serpyco_rs import (
    AuthorAssociation,
    Issue,
    IssueLabel,
    IssueState,
    IssueStateReason,
    Milestone,
    MilestoneState,
    Reactions,
    User,
)


def _dt(value: str) -> datetime:
    # `fromisoformat` handles the trailing `Z` from 3.11 on; the benchmark runs 3.12.
    return datetime.fromisoformat(value)


def _opt_dt(value: Optional[str]) -> Optional[datetime]:
    return None if value is None else _dt(value)


def _user(d: dict[str, Any]) -> User:
    return User(
        login=d['login'],
        id=d['id'],
        node_id=d['node_id'],
        avatar_url=d['avatar_url'],
        gravatar_id=d['gravatar_id'],
        url=d['url'],
        html_url=d['html_url'],
        followers_url=d['followers_url'],
        following_url=d['following_url'],
        gists_url=d['gists_url'],
        starred_url=d['starred_url'],
        subscriptions_url=d['subscriptions_url'],
        organizations_url=d['organizations_url'],
        repos_url=d['repos_url'],
        events_url=d['events_url'],
        received_events_url=d['received_events_url'],
        type=d['type'],
        site_admin=d['site_admin'],
        name=d.get('name'),
        email=d.get('email'),
        starred_at=_opt_dt(d.get('starred_at')),
    )


def _opt_user(d: Optional[dict[str, Any]]) -> Optional[User]:
    return None if d is None else _user(d)


def _label(d: Any) -> Any:
    if isinstance(d, str):
        return d
    return IssueLabel(
        id=d['id'],
        node_id=d['node_id'],
        url=d['url'],
        name=d['name'],
        description=d['description'],
        color=d['color'],
        default=d['default'],
    )


def _milestone(d: Optional[dict[str, Any]]) -> Optional[Milestone]:
    if d is None:
        return None
    return Milestone(
        url=d['url'],
        html_url=d['html_url'],
        labels_url=d['labels_url'],
        id=d['id'],
        node_id=d['node_id'],
        number=d['number'],
        title=d['title'],
        description=d['description'],
        creator=_opt_user(d['creator']),
        open_issues=d['open_issues'],
        closed_issues=d['closed_issues'],
        created_at=_dt(d['created_at']),
        updated_at=_dt(d['updated_at']),
        closed_at=_opt_dt(d['closed_at']),
        due_on=_opt_dt(d['due_on']),
        state=MilestoneState(d.get('state', 'open')),
    )


def _reactions(d: Optional[dict[str, Any]]) -> Optional[Reactions]:
    if d is None:
        return None
    return Reactions(
        url=d['url'],
        total_count=d['total_count'],
        plus_one=d['+1'],
        minus_one=d['-1'],
        laugh=d['laugh'],
        confused=d['confused'],
        heart=d['heart'],
        hooray=d['hooray'],
        eyes=d['eyes'],
        rocket=d['rocket'],
    )


def load(d: dict[str, Any]) -> Issue:
    assignees = d['assignees']
    state_reason = d['state_reason']
    return Issue(
        id=d['id'],
        node_id=d['node_id'],
        url=d['url'],
        repository_url=d['repository_url'],
        labels_url=d['labels_url'],
        comments_url=d['comments_url'],
        events_url=d['events_url'],
        html_url=d['html_url'],
        number=d['number'],
        state=IssueState(d['state']),
        state_reason=None if state_reason is None else IssueStateReason(state_reason),
        title=d['title'],
        body=d['body'],
        user=_opt_user(d['user']),
        labels=[_label(x) for x in d['labels']],
        assignee=_opt_user(d['assignee']),
        assignees=None if assignees is None else [_user(x) for x in assignees],
        milestone=_milestone(d['milestone']),
        locked=d['locked'],
        active_lock_reason=d['active_lock_reason'],
        comments=d['comments'],
        closed_at=_opt_dt(d['closed_at']),
        created_at=_dt(d['created_at']),
        updated_at=_dt(d['updated_at']),
        closed_by=_opt_user(d['closed_by']),
        author_association=AuthorAssociation(d['author_association']),
        draft=d.get('draft', False),
        body_html=d.get('body_html'),
        body_text=d.get('body_text'),
        timeline_url=d.get('timeline_url'),
        reactions=_reactions(d['reactions']),
    )
