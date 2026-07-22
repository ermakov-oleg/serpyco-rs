import pytest
from serpyco_rs import _type_info
from serpyco_rs._impl import Serializer as RustSerializer


def test_not_set_behaves_like_missing_value_sentinel():
    assert _type_info.NOT_SET is type(_type_info.NOT_SET).token
    assert _type_info.NOT_SET.name == 'token'


def test_union_type_repr_uses_ref_name():
    type_info = _type_info.UnionType(
        ref_name='int | str',
        item_types=[_type_info.IntegerType(), _type_info.StringType()],
    )

    assert type_info.repr == 'int | str'


def test_rust_serializer_fails_when_recursive_type_is_not_resolved():
    type_info = _type_info.RecursionHolder(name='Node', state_key='node', meta={})

    with pytest.raises(RuntimeError, match="Recursive type not resolved: KeyError: 'node'"):
        RustSerializer(type_info, False, 1000)
