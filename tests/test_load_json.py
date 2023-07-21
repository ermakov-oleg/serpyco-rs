import pytest
from serpyco_rs import SchemaValidationError, Serializer, ValidationError


def test_load_invalid_json__expected_validation_error():
    ser = Serializer(str)
    with pytest.raises(ValidationError) as exc_info:
        ser.load_json('{"a', validate=True)

    assert exc_info.value.args[0] == 'Invalid JSON string: EOF while parsing a string at line 1 column 3'


def test_load_json_invalid_type__expected_validation_error():
    ser = Serializer(str)
    with pytest.raises(SchemaValidationError) as exc_info:
        ser.load_json('123', validate=True)

    assert exc_info.value.errors[0].message == '123 is not of type "string"'
