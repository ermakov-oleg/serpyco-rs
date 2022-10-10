import uuid
from dataclasses import dataclass
from datetime import time, datetime, date
from decimal import Decimal
from enum import Enum
from typing import Annotated, Optional, Any

import pytest

from serpyco_rs import ValidationError
from serpyco_rs._describe import describe_type
from serpyco_rs._json_schema import to_json_schema, JsonschemaRSValidator
from serpyco_rs.exceptions import ErrorItem
from serpyco_rs.metadata import MinLength, MaxLength, Min, Max


def test_validate():
    @dataclass
    class Inner:
        baz: str

    @dataclass
    class A:
        foo: int
        bar: Inner

    v = JsonschemaRSValidator(to_json_schema(describe_type(A)).dump())

    with pytest.raises(ValidationError) as exc_info:
        v.validate({"foo": "1", "bar": {"buz": None}, "qux": 0})

    assert exc_info.value.errors == [
        ErrorItem(
            message='"baz" is a required property',
            instance_path="bar",
            schema_path="properties/bar/required",
        ),
        ErrorItem(
            message='"1" is not of type "integer"',
            instance_path="foo",
            schema_path="properties/foo/type",
        ),
    ]


class EnumTest(Enum):
    foo = 'foo'
    bar = 'bar'


@dataclass
class EntityTest:
    key: str


@pytest.mark.parametrize(['cls', 'value'], (
    (bool, True),
    (bool, False),
    (str, ""),
    (Annotated[str, MinLength(1), MaxLength(3)], "12"),
    (int, -99),
    (Annotated[int, Min(1), Max(1000)], 99),
    # (bytes, b'xx'),  # todo: fix bytes validation
    (float, 1.3),
    (Annotated[float, Min(0), Max(0.4)], 0.1),
    (Decimal, "0.1"),  # support str
    (Decimal, 0.1),    # or int input
    (uuid.UUID, str(uuid.uuid4())),  # support only str input
    # (time, '12:34'),  # todo: fix
    # (time, '12:34Z'),  # todo: fix
    # (time, '12:34:56'),  # todo: fix
    # (time, '12:34+0300'),  # todo: fix
    # (time, '12:34+03:00'),  # todo: fix
    (time, '12:34:00+03:00'),
    (time, '12:34:56.000078+03:00'),
    (time, '12:34:56.000078+00:00'),
    # todo: add datetime exemplars
    (datetime, '2022-10-10T14:23:43.123456+00:00'),
    (date, '2020-07-17'),
    (EnumTest, 'foo'),
    (Optional[int], None),
    (int | None, None),
    (Optional[int], 1),
    (int | None, 2),
    (EntityTest, {'key': 'val'}),
    (list[int], [1, 2]),
    (dict[str, int], {'a': 1}),
    (tuple[str, int, bool], ['1', 2, True]),
    (Any, ['1', 2, True]),
    (Any, {}),
))
def test_validate(cls, value):
    v = JsonschemaRSValidator(to_json_schema(describe_type(cls)).dump())
    v.validate(value)
