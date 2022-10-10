from dataclasses import dataclass

import pytest

from serpyco_rs import ValidationError
from serpyco_rs._describe import describe_type
from serpyco_rs._json_schema import (
    to_json_schema,
    ValicoValidator,
)


def test_validate():
    @dataclass
    class Inner:
        baz: str

    @dataclass
    class A:
        foo: int
        bar: Inner

    v = ValicoValidator(to_json_schema(describe_type(A)).dump())

    with pytest.raises(ValidationError) as exc_info:
        v.validate({"foo": "1", "bar": {"buz": None}, "qux": 0})

    assert exc_info.value.args == (
        [
            {
                "code": "required",
                "path": "/bar/baz",
                "title": "This property is required",
            },
            {
                "code": "wrong_type",
                "detail": "The value must be integer",
                "path": "/foo",
                "title": "Type of the value is wrong",
            },
        ],
    )
