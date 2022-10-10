from dataclasses import dataclass

import pytest

from serpyco_rs import ValidationError
from serpyco_rs._describe import describe_type
from serpyco_rs._json_schema import to_json_schema, JsonschemaRSValidator
from serpyco_rs.exceptions import ErrorItem


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
