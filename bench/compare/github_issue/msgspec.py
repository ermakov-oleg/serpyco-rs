import datetime
import enum
from typing import Optional, Union

import msgspec


class IssueState(enum.Enum):
    OPEN = 'open'
    CLOSED = 'closed'


class MilestoneState(enum.Enum):
    OPEN = 'open'
    CLOSED = 'closed'


class IssueStateReason(enum.Enum):
    COMPLETED = 'completed'
    REOPENED = 'reopened'
    NOT_PLANNED = 'not_planned'


class AuthorAssociation(enum.Enum):
    COLLABORATOR = 'COLLABORATOR'
    CONTRIBUTOR = 'CONTRIBUTOR'
    FIRST_TIMER = 'FIRST_TIMER'
    FIRST_TIME_CONTRIBUTOR = 'FIRST_TIME_CONTRIBUTOR'
    MANNEQUIN = 'MANNEQUIN'
    MEMBER = 'MEMBER'
    NONE = 'NONE'
    OWNER = 'OWNER'


class User(msgspec.Struct):
    login: str
    id: int
    node_id: str
    avatar_url: str
    gravatar_id: Optional[str]
    url: str
    html_url: str
    followers_url: str
    following_url: str
    gists_url: str
    starred_url: str
    subscriptions_url: str
    organizations_url: str
    repos_url: str
    events_url: str
    received_events_url: str
    type: str
    site_admin: bool
    name: Optional[str] = None
    email: Optional[str] = None
    starred_at: Optional[datetime.datetime] = None


class IssueLabel(msgspec.Struct):
    id: int
    node_id: str
    url: str
    name: str
    description: Optional[str]
    color: Optional[str]
    default: bool


class Milestone(msgspec.Struct):
    url: str
    html_url: str
    labels_url: str
    id: int
    node_id: str
    number: int
    title: str
    description: Optional[str]
    creator: Optional[User]
    open_issues: int
    closed_issues: int
    created_at: datetime.datetime
    updated_at: datetime.datetime
    closed_at: Optional[datetime.datetime]
    due_on: Optional[datetime.datetime]
    state: MilestoneState = MilestoneState.OPEN


class Reactions(msgspec.Struct):
    url: str
    total_count: int
    plus_one: int = msgspec.field(name='+1')
    minus_one: int = msgspec.field(name='-1')
    laugh: int
    confused: int
    heart: int
    hooray: int
    eyes: int
    rocket: int


class Issue(msgspec.Struct):
    id: int
    node_id: str
    url: str
    repository_url: str
    labels_url: str
    comments_url: str
    events_url: str
    html_url: str
    number: int
    state: IssueState
    state_reason: Optional[IssueStateReason]
    title: str
    body: Optional[str]
    user: Optional[User]
    labels: list[Union[IssueLabel, str]]
    assignee: Optional[User]
    assignees: Optional[list[User]]
    milestone: Optional[Milestone]
    locked: bool
    active_lock_reason: Optional[str]
    comments: int
    closed_at: Optional[datetime.datetime]
    created_at: datetime.datetime
    updated_at: datetime.datetime
    closed_by: Optional[User]
    author_association: AuthorAssociation
    draft: bool = False
    body_html: Optional[str] = None
    body_text: Optional[str] = None
    timeline_url: Optional[str] = None
    reactions: Optional[Reactions] = None


def load(data: bytes) -> Issue:
    return msgspec.json.decode(data, type=Issue)


def dump(obj: Issue) -> bytes:
    return msgspec.json.encode(obj)
