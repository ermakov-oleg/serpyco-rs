import sys
from dataclasses import dataclass
from typing import Annotated, Literal

import pytest
from serpyco_rs import Serializer
from serpyco_rs.metadata import Discriminator


@pytest.mark.skipif(sys.version_info < (3, 12), reason='TypeAliasType available after 3.12')
def test_serialize__new_type_var_recursive_generic_dataclass__correct():
    exec(
        """
from dataclasses import dataclass
from typing import Annotated, Literal

@dataclass(kw_only=True)
class Foo[T]:
    type: Literal['Foo'] = 'Foo'
    children_field: list[T]

@dataclass(kw_only=True)
class Bar[T]:
    type: Literal['Bar'] = 'Bar'
    children_field: list[T]

    field: int | None = None

type Core = Annotated[Foo[Core] | Bar[Core], Discriminator('type')]
    """,
        globals(),
    )

    serializer = Serializer(Core, camelcase_fields=True, omit_none=True)

    assert serializer.dump(Bar(children_field=[Foo(children_field=[Bar(children_field=[], field=1)])])) == {
        'childrenField': [{'childrenField': [{'childrenField': [], 'field': 1, 'type': 'Bar'}], 'type': 'Foo'}],
        'type': 'Bar',
    }
