import sys
import uuid
from dataclasses import dataclass
from datetime import time, datetime, date
from decimal import Decimal
from enum import Enum
from typing import Annotated, Optional, Any
from unittest import mock

import pytest

from serpyco_rs._describe import describe_type
from serpyco_rs._json_schema import to_json_schema, JsonschemaRSValidator
from serpyco_rs.exceptions import ErrorItem, SchemaValidationError
from serpyco_rs.metadata import MinLength, MaxLength, Min, Max


class EnumTest(Enum):
    foo = "foo"
    bar = "bar"


@dataclass
class EntityTest:
    key: str


@pytest.mark.parametrize(
    ["cls", "value"],
    (
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
        (Decimal, 0.1),  # or int input
        (Decimal, "NaN"),  # or int input
        (uuid.UUID, str(uuid.uuid4())),  # support only str input
        (time, "12:34"),
        (time, "12:34Z"),
        (time, "12:34:56"),
        (time, "12:34+0300"),
        (time, "12:34+03:00"),
        (time, "12:34:00+03:00"),
        (time, "12:34:56.000078+03:00"),
        (time, "12:34:56.000078+00:00"),
        # todo: add datetime exemplars
        (datetime, "2022-10-10T14:23:43"),
        (datetime, "2022-10-10T14:23:43.123456"),
        (datetime, "2022-10-10T14:23:43.123456Z"),
        (datetime, "2022-10-10T14:23:43.123456+00:00"),
        (datetime, "2022-10-10T14:23:43.123456-30:00"),
        (date, "2020-07-17"),
        (EnumTest, "foo"),
        (Optional[int], None),
        (Optional[int], 1),
        (EntityTest, {"key": "val"}),
        (list[int], [1, 2]),
        (dict[str, int], {"a": 1}),
        (tuple[str, int, bool], ["1", 2, True]),
        (Any, ["1", 2, True]),
        (Any, {}),
    ),
)
def test_validate(cls, value):
    v = JsonschemaRSValidator(to_json_schema(describe_type(cls)).dump())
    v.validate(value)


if sys.version_info >= (3, 10):

    @pytest.mark.parametrize(
        ["cls", "value"],
        (
            (Optional[int], None),
            (int | None, None),
            (int | None, 2),
        ),
    )
    def test_validate(cls, value):
        v = JsonschemaRSValidator(to_json_schema(describe_type(cls)).dump())
        v.validate(value)


def _mk_e(m=mock.ANY, ip=mock.ANY, sp=mock.ANY) -> ErrorItem:
    return ErrorItem(message=m, instance_path=ip, schema_path=sp)


@pytest.mark.parametrize(
    ["cls", "value", "err"],
    (
        (bool, 1, _mk_e(m='1 is not of type "boolean"')),
        (str, 1, _mk_e(m='1 is not of type "string"')),
        (
            Annotated[str, MinLength(2)],
            "a",
            _mk_e(m='"a" is shorter than 2 characters', sp="minLength"),
        ),
        (
            Annotated[str, MaxLength(2)],
            "aaa",
            _mk_e(m='"aaa" is longer than 2 characters', sp="maxLength"),
        ),
        (int, 9.1, _mk_e(m='9.1 is not of type "integer"')),
        (int, "9", _mk_e(m='"9" is not of type "integer"')),
        (
            Annotated[int, Min(1)],
            0,
            _mk_e(m="0 is less than the minimum of 1", sp="minimum"),
        ),
        (
            Annotated[int, Max(1)],
            10,
            _mk_e(m="10 is greater than the maximum of 1", sp="maximum"),
        ),
        (float, None, _mk_e(m='null is not of type "number"')),
        (
            Annotated[float, Min(1)],
            0.1,
            _mk_e(m="0.1 is less than the minimum of 1", sp="minimum"),
        ),
        (
            Annotated[float, Max(1)],
            10.1,
            _mk_e(m="10.1 is greater than the maximum of 1", sp="maximum"),
        ),
        # (uuid.UUID, "asd", ''),  # todo: validation don't work
        (time, "12:34:a", _mk_e(sp="pattern")),
        (datetime, "2022-10-10//12", _mk_e(sp="pattern")),
        (date, "17-02-2022", _mk_e(sp="pattern")),
        (EnumTest, "buz", _mk_e(m='"buz" is not one of ["foo","bar"]', sp="enum")),
        (
            Optional[int],
            "foo",
            _mk_e(m='"foo" is not valid under any of the given schemas', sp="anyOf"),
        ),
        (EntityTest, {}, _mk_e(m='"key" is a required property', sp="required")),
        (
            list[int],
            [1, "1"],
            _mk_e(m='"1" is not of type "integer"', ip="1", sp="items/type"),
        ),
        (
            dict[str, int],
            {"a": "1"},
            _mk_e(
                m='"1" is not of type "integer"', ip="a", sp="additionalProperties/type"
            ),
        ),
        (
            tuple[str, int, bool],
            ["1"],
            _mk_e(m='["1"] has less than 3 items', sp="minItems"),
        ),
        (
            tuple[str, int, bool],
            ["1", 1, True, 0],
            _mk_e(m='["1",1,true,0] has more than 3 items', sp="maxItems"),
        ),
        # (tuple[str, bool], [1, '1'], ''),   # todo: validation don't work
    ),
)
def test_validate__validation_error(cls, value, err):
    v = JsonschemaRSValidator(to_json_schema(describe_type(cls)).dump())

    with pytest.raises(SchemaValidationError) as exc_info:
        v.validate(value)

    assert exc_info.value.errors == [err]


def test_validate__error_format():
    @dataclass
    class Inner:
        baz: str

    @dataclass
    class A:
        foo: int
        bar: Inner

    v = JsonschemaRSValidator(to_json_schema(describe_type(A)).dump())

    with pytest.raises(SchemaValidationError) as exc_info:
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
