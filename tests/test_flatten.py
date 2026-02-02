from dataclasses import dataclass
from typing import Annotated, Any, Optional

import pytest
from serpyco_rs import SchemaValidationError, Serializer
from serpyco_rs.metadata import Alias, Flatten
from typing_extensions import Never, TypedDict


@dataclass
class Address:
    street: str
    city: str
    country: str


@dataclass
class Person:
    name: str
    age: int
    address: Annotated[Address, Flatten]
    extra: Annotated[dict[str, Any], Flatten]


def test_flatten_serialization():
    serializer = Serializer(Person)

    address = Address(street='123 Main St', city='New York', country='USA')
    person = Person(
        name='John Doe',
        age=30,
        address=address,
        extra={'phone': '555-1234', 'email': 'john@example.com'},
    )
    raw = {
        'name': 'John Doe',
        'age': 30,
        'street': '123 Main St',
        'city': 'New York',
        'country': 'USA',
        'phone': '555-1234',
        'email': 'john@example.com',
    }

    assert serializer.dump(person) == raw
    assert serializer.load(raw) == person


def test_flatten_first_position():
    @dataclass
    class FlattenFirst:
        metadata: Annotated[dict[str, Any], Flatten]
        name: str
        age: int

    serializer = Serializer(FlattenFirst)

    obj = FlattenFirst(metadata={'version': '1.0', 'author': 'test'}, name='John', age=30)
    raw = {'name': 'John', 'age': 30, 'version': '1.0', 'author': 'test'}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_flatten_last_position():
    @dataclass
    class FlattenLast:
        name: str
        age: int
        metadata: Annotated[dict[str, Any], Flatten]

    serializer = Serializer(FlattenLast)

    obj = FlattenLast(name='Bob', age=35, metadata={'location': 'NYC', 'timezone': 'EST'})
    raw = {'name': 'Bob', 'age': 35, 'location': 'NYC', 'timezone': 'EST'}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_multiple_dict_flatten_validation():
    @dataclass
    class MultipleFlatten:
        name: str
        config: Annotated[dict[str, Any], Flatten]
        age: int
        extra: Annotated[dict[str, Any], Flatten]
        status: str

    with pytest.raises(
        RuntimeError, match=r"Multiple dict flatten fields are not allowed in <class '.+\.MultipleFlatten'>"
    ):
        Serializer(MultipleFlatten)


@dataclass
class NestedA:
    field_a1: str
    field_a2: int


@dataclass
class NestedB:
    field_b1: str
    field_b2: bool


def test_multiple_struct_flatten():
    @dataclass
    class MultipleStructFlatten:
        id: int
        struct_a: Annotated[NestedA, Flatten]
        name: str
        struct_b: Annotated[NestedB, Flatten]
        status: str

    serializer = Serializer(MultipleStructFlatten)

    obj = MultipleStructFlatten(
        id=1,
        struct_a=NestedA(field_a1='value_a1', field_a2=42),
        name='test',
        struct_b=NestedB(field_b1='value_b1', field_b2=True),
        status='ok',
    )
    raw = {
        'id': 1,
        'field_a1': 'value_a1',
        'field_a2': 42,
        'name': 'test',
        'field_b1': 'value_b1',
        'field_b2': True,
        'status': 'ok',
    }

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_empty_dict_flatten():
    @dataclass
    class EmptyDictFlatten:
        name: str
        empty: Annotated[dict[str, Any], Flatten]

    serializer = Serializer(EmptyDictFlatten)

    obj = EmptyDictFlatten(name='test', empty={})
    raw = {'name': 'test'}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_flatten_with_camelcase():
    @dataclass
    class CamelCaseStruct:
        long_field_name: str
        another_field: int

    @dataclass
    class WithCamelCase:
        user_name: str
        nested_data: Annotated[CamelCaseStruct, Flatten]

    serializer = Serializer(WithCamelCase, camelcase_fields=True)

    obj = WithCamelCase(user_name='test', nested_data=CamelCaseStruct(long_field_name='value', another_field=42))
    raw = {'userName': 'test', 'longFieldName': 'value', 'anotherField': 42}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_flatten_with_alias():
    @dataclass
    class AliasedStruct:
        internal_name: Annotated[str, Alias('external_name')]
        value: int

    @dataclass
    class WithAlias:
        id: int
        data: Annotated[AliasedStruct, Flatten]

    serializer = Serializer(WithAlias)

    obj = WithAlias(id=123, data=AliasedStruct(internal_name='test', value=456))
    raw = {'id': 123, 'external_name': 'test', 'value': 456}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_flatten_dict_with_camelcase():
    @dataclass
    class CamelCaseWithDict:
        user_name: str
        meta_data: Annotated[dict[str, Any], Flatten]

    serializer = Serializer(CamelCaseWithDict, camelcase_fields=True)

    obj = CamelCaseWithDict(user_name='test', meta_data={'api_key': 'secret', 'log_level': 'debug'})
    raw = {'userName': 'test', 'api_key': 'secret', 'log_level': 'debug'}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_flatten_with_omit_none():
    @dataclass
    class OptionalStruct:
        required_field: str
        optional_field: Optional[str] = None

    @dataclass
    class WithOmitNone:
        name: str
        data: Annotated[OptionalStruct, Flatten]

    serializer = Serializer(WithOmitNone, omit_none=True)

    obj = WithOmitNone(name='test', data=OptionalStruct(required_field='value', optional_field=None))
    raw = {'name': 'test', 'required_field': 'value'}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_struct_vs_regular_field_conflict():
    @dataclass
    class ConflictStruct:
        name: str  # This conflicts with regular field
        value: int

    @dataclass
    class WithConflict:
        name: str
        nested: Annotated[ConflictStruct, Flatten]

    with pytest.raises(
        RuntimeError, match=r"Field name conflict in <class '.+\.WithConflict'>: 'name' from flattened struct field"
    ):
        Serializer(WithConflict)


def test_struct_field_conflict_resolved_with_alias():
    @dataclass
    class ConflictStruct:
        name: Annotated[str, Alias('nested_name')]  # Resolved with alias
        value: int

    @dataclass
    class WithConflictResolved:
        name: str
        nested: Annotated[ConflictStruct, Flatten]

    serializer = Serializer(WithConflictResolved)

    obj = WithConflictResolved(name='regular_name', nested=ConflictStruct(name='nested_value', value=42))
    raw = {'name': 'regular_name', 'nested_name': 'nested_value', 'value': 42}

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_struct_flatten_fields_conflict():
    @dataclass
    class StructA:
        common_field: str
        field_a: int

    @dataclass
    class StructB:
        common_field: int  # Same name but different type
        field_b: bool

    @dataclass
    class WithStructConflict:
        id: int
        struct_a: Annotated[StructA, Flatten]
        struct_b: Annotated[StructB, Flatten]

    # This should work since we don't validate conflicts between struct flatten fields
    # The behavior is that later fields overwrite earlier ones
    serializer = Serializer(WithStructConflict)

    obj = WithStructConflict(
        id=1,
        struct_a=StructA(common_field='string_value', field_a=42),
        struct_b=StructB(common_field=123, field_b=True),
    )

    assert serializer.dump(obj) == {
        'id': 1,
        'field_a': 42,
        'field_b': True,
        'common_field': 123,  # From struct_b (int)
    }


def test_deep_nested_flatten():
    @dataclass
    class Level3:
        level3_field: str

    @dataclass
    class Level2:
        level2_field: str
        level3: Annotated[Level3, Flatten]

    @dataclass
    class Level1:
        level1_field: str
        level2: Annotated[Level2, Flatten]

    serializer = Serializer(Level1)

    obj = Level1(level1_field='L1', level2=Level2(level2_field='L2', level3=Level3(level3_field='L3')))
    raw = {
        'level1_field': 'L1',
        'level2_field': 'L2',
        'level3_field': 'L3',
    }

    assert serializer.dump(obj) == raw
    assert serializer.load(raw) == obj


def test_flatten_unsupported_type_error():
    @dataclass
    class WithBadFlatten:
        name: str
        bad_field: Annotated[str, Flatten]  # str is not a dict

    with pytest.raises(RuntimeError, match="Flatten field 'bad_field' has type 'StringType' which cannot be flattened"):
        Serializer(WithBadFlatten)


def test_typeddict_flatten_basic():
    class NestedTypedDict(TypedDict):
        nested_field1: str
        nested_field2: int

    class MainTypedDict(TypedDict):
        main_field: str
        nested: Annotated[NestedTypedDict, Flatten]

    serializer = Serializer(MainTypedDict)

    data = {'main_field': 'main_value', 'nested': {'nested_field1': 'nested_value1', 'nested_field2': 42}}
    expected = {'main_field': 'main_value', 'nested_field1': 'nested_value1', 'nested_field2': 42}

    assert serializer.dump(data) == expected
    assert serializer.load(expected) == data


def test_typeddict_flatten_with_dict():
    class WithDictFlatten(TypedDict):
        name: str
        metadata: Annotated[dict[str, Any], Flatten]

    serializer = Serializer(WithDictFlatten)

    data = {'name': 'test', 'metadata': {'key1': 'value1', 'key2': 'value2'}}
    expected = {'name': 'test', 'key1': 'value1', 'key2': 'value2'}

    assert serializer.dump(data) == expected
    assert serializer.load(expected) == data


def test_typeddict_multiple_struct_flatten():
    class StructA(TypedDict):
        field_a1: str
        field_a2: int

    class StructB(TypedDict):
        field_b1: str
        field_b2: bool

    class MultipleStructFlatten(TypedDict):
        id: int
        struct_a: Annotated[StructA, Flatten]
        name: str
        struct_b: Annotated[StructB, Flatten]

    serializer = Serializer(MultipleStructFlatten)

    data = {
        'id': 1,
        'struct_a': {'field_a1': 'value_a1', 'field_a2': 42},
        'name': 'test',
        'struct_b': {'field_b1': 'value_b1', 'field_b2': True},
    }
    expected = {
        'id': 1,
        'field_a1': 'value_a1',
        'field_a2': 42,
        'name': 'test',
        'field_b1': 'value_b1',
        'field_b2': True,
    }

    assert serializer.dump(data) == expected
    assert serializer.load(expected) == data


def test_typeddict_mixed_dataclass_and_typeddict_flatten():
    @dataclass
    class DataclassStruct:
        dc_field1: str
        dc_field2: int

    class TypedDictStruct(TypedDict):
        td_field1: str
        td_field2: bool

    class MixedFlatten(TypedDict):
        id: int
        dc_data: Annotated[DataclassStruct, Flatten]
        td_data: Annotated[TypedDictStruct, Flatten]

    serializer = Serializer(MixedFlatten)

    data = {
        'id': 1,
        'dc_data': DataclassStruct(dc_field1='dc_value', dc_field2=42),
        'td_data': {'td_field1': 'td_value', 'td_field2': True},
    }
    expected = {'id': 1, 'dc_field1': 'dc_value', 'dc_field2': 42, 'td_field1': 'td_value', 'td_field2': True}

    assert serializer.dump(data) == expected
    assert serializer.load(expected) == data


def test_flatten_dict_never_forbids_extra_properties():
    """Test that dict[str, Never] with Flatten forbids extra properties"""

    @dataclass
    class StrictPerson:
        name: str
        age: int
        _forbid_extra: Annotated[dict[str, Never], Flatten]

    serializer = Serializer(StrictPerson)

    # Test serialization
    person = StrictPerson(name='John', age=30, _forbid_extra={})
    result = serializer.dump(person)
    assert result == {'name': 'John', 'age': 30}

    # Test valid deserialization (no extra fields)
    valid_data = {'name': 'John', 'age': 30}
    loaded = serializer.load(valid_data)
    assert loaded == person

    # Test invalid deserialization (with extra fields)
    invalid_data = {'name': 'John', 'age': 30, 'extra_field': 'value'}
    with pytest.raises(SchemaValidationError, match='extra_field'):
        serializer.load(invalid_data)


def test_flatten_dict_never_with_struct_flatten():
    """Test dict[str, Never] combined with struct flatten"""

    @dataclass
    class Address:
        street: str
        city: str

    @dataclass
    class StrictPersonWithAddress:
        name: str
        address: Annotated[Address, Flatten]
        _forbid_extra: Annotated[dict[str, Never], Flatten]

    serializer = Serializer(StrictPersonWithAddress)

    # Test serialization
    person = StrictPersonWithAddress(name='John', address=Address(street='123 Main St', city='NYC'), _forbid_extra={})
    result = serializer.dump(person)
    assert result == {'name': 'John', 'street': '123 Main St', 'city': 'NYC'}

    # Test valid deserialization
    valid_data = {'name': 'John', 'street': '123 Main St', 'city': 'NYC'}
    loaded = serializer.load(valid_data)
    assert loaded == person

    # Test invalid deserialization (extra field should fail)
    invalid_data = {'name': 'John', 'street': '123 Main St', 'city': 'NYC', 'extra': 'value'}
    with pytest.raises(SchemaValidationError, match='extra'):
        serializer.load(invalid_data)


def test_typeddict_flatten_dict_never():
    """Test dict[str, Never] with Flatten in TypedDict"""

    class StrictTypedDict(TypedDict):
        name: str
        _forbid_extra: Annotated[dict[str, Never], Flatten]

    serializer = Serializer(StrictTypedDict)

    # Test serialization
    data = {'name': 'John', '_forbid_extra': {}}
    result = serializer.dump(data)
    assert result == {'name': 'John'}

    # Test valid deserialization
    valid_data = {'name': 'John'}
    loaded = serializer.load(valid_data)
    assert loaded == data

    # Test invalid deserialization
    invalid_data = {'name': 'John', 'extra': 'value'}
    with pytest.raises(SchemaValidationError, match='extra'):
        serializer.load(invalid_data)
