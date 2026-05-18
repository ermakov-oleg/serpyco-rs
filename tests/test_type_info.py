import pytest
from serpyco_rs import _type_info
from serpyco_rs._impl import Serializer as RustSerializer


def test_not_set_behaves_like_missing_value_sentinel():
    assert _type_info.NOT_SET is type(_type_info.NOT_SET).token
    assert _type_info.NOT_SET.name == 'token'


def test_recursion_holder_returns_resolved_type():
    inner_type = _type_info.StringType()
    holder = _type_info.RecursionHolder(name='Node', state_key='node', meta={'node': inner_type})

    assert holder.get_inner_type() is inner_type


def test_recursion_holder_fails_when_type_is_not_resolved():
    holder = _type_info.RecursionHolder(name='Node', state_key='node', meta={})

    with pytest.raises(RuntimeError, match="Recursive type not resolved: 'node'"):
        holder.get_inner_type()


def test_union_type_repr_uses_ref_name():
    type_info = _type_info.UnionType(
        ref_name='int | str',
        item_types=[_type_info.IntegerType(), _type_info.StringType()],
    )

    assert type_info.repr == 'int | str'


def test_rust_serializer_fails_when_recursive_type_is_not_resolved():
    type_info = _type_info.RecursionHolder(name='Node', state_key='node', meta={})

    with pytest.raises(RuntimeError, match="Recursive type not resolved: KeyError: 'node'"):
        RustSerializer(type_info, False)
