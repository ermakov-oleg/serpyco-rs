from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Any, Union
from typing import Optional, Annotated

from serpyco_rs.metadata import Alias
import serpyco_rs

from ._utils import get_dataclass_args


class IssueState(Enum):
    OPEN = 'open'
    CLOSED = 'closed'


class MilestoneState(Enum):
    OPEN = 'open'
    CLOSED = 'closed'


class IssueStateReason(Enum):
    COMPLETED = 'completed'
    REOPENED = 'reopened'
    NOT_PLANNED = 'not_planned'


class AuthorAssociation(Enum):
    COLLABORATOR = 'COLLABORATOR'
    CONTRIBUTOR = 'CONTRIBUTOR'
    FIRST_TIMER = 'FIRST_TIMER'
    FIRST_TIME_CONTRIBUTOR = 'FIRST_TIME_CONTRIBUTOR'
    MANNEQUIN = 'MANNEQUIN'
    MEMBER = 'MEMBER'
    NONE = 'NONE'
    OWNER = 'OWNER'


@dataclass(**get_dataclass_args())
class User:
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
    starred_at: Optional[datetime] = None


@dataclass(**get_dataclass_args())
class IssueLabel:
    id: int
    node_id: str
    url: str
    name: str
    description: Optional[str]
    color: Optional[str]
    default: bool


@dataclass(**get_dataclass_args())
class Milestone:
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
    created_at: datetime
    updated_at: datetime
    closed_at: Optional[datetime]
    due_on: Optional[datetime]
    state: MilestoneState = MilestoneState.OPEN


@dataclass(**get_dataclass_args())
class Reactions:
    url: str
    total_count: int
    plus_one: Annotated[int, Alias('+1')]
    minus_one: Annotated[int, Alias('-1')]
    laugh: int
    confused: int
    heart: int
    hooray: int
    eyes: int
    rocket: int


@dataclass(**get_dataclass_args())
class Issue:
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
    closed_at: Optional[datetime]
    created_at: datetime
    updated_at: datetime
    closed_by: Optional[User]
    author_association: AuthorAssociation
    draft: bool = False
    body_html: Optional[str] = None
    body_text: Optional[str] = None
    timeline_url: Optional[str] = None
    reactions: Optional[Reactions] = None


_serializer = serpyco_rs.Serializer(Issue)


def load(data: dict[str, Any]) -> Issue:
    return _serializer.load(data)


def dump(obj: Issue) -> dict[str, Any]:
    return _serializer.dump(obj)
