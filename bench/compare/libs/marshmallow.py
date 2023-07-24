from typing import Any

import marshmallow_dataclass

from .base import Dataclass, test_object


_schema = marshmallow_dataclass.class_schema(Dataclass)()

test_object = test_object


def load(data: dict[str, Any], validate: bool = True) -> Dataclass:
    return _schema.load(data)


def load_json(data: str, validate: bool = True) -> Dataclass:
    # todo: use orjson as render_module
    return _schema.loads(data)


def dump(obj: Dataclass) -> dict[str, Any]:
    return _schema.dump(obj)
