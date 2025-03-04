from dataclasses import dataclass
from typing import Annotated, Generic, Literal, TypeVar, Optional, Union

import pytest
import sys

from serpyco_rs import Serializer
from serpyco_rs.metadata import Discriminator


TWidget_co = TypeVar('TWidget_co', bound='BaseWidget', covariant=True)


@dataclass
class BaseWidget(Generic[TWidget_co]):
    type: str
    childrens: Optional[list[TWidget_co]] = None


@dataclass
class Widget1(BaseWidget[TWidget_co]):
    type: Literal['Widget1'] = 'Widget1'


@dataclass
class Widget2(BaseWidget[TWidget_co]):
    type: Literal['Widget2'] = 'Widget2'
    some_field: Optional[str] = None


Widget = Annotated[Union[Widget1['Widget'], Widget2['Widget']], Discriminator('type')]


@pytest.mark.skipif(sys.version_info < (3, 11), reason='requires python3.11 or higher')
def test_recursive_generics():
    serializer = Serializer(Widget)

    obj = Widget1(type='Widget1', childrens=[Widget2(type='Widget2', some_field='some_value')])

    data = {'type': 'Widget1', 'childrens': [{'type': 'Widget2', 'some_field': 'some_value', 'childrens': None}]}

    assert serializer.dump(obj) == data
    assert serializer.load(data) == obj


@pytest.mark.skipif(sys.version_info < (3, 11), reason='requires python3.11 or higher')
def test_recursive_generics_propagates_annotations():
    serializer = Serializer(Widget, camelcase_fields=True)

    obj = Widget1(type='Widget1', childrens=[Widget2(type='Widget2', some_field='some_value', childrens=[])])

    data = {'type': 'Widget1', 'childrens': [{'type': 'Widget2', 'someField': 'some_value', 'childrens': []}]}

    assert serializer.dump(obj) == data
    assert serializer.load(data) == obj
