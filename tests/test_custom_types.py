from dataclasses import dataclass
from ipaddress import IPv4Address, AddressValueError
from typing import Optional

import pytest

from serpyco_rs import Serializer, SchemaValidationError, ErrorItem
from serpyco_rs._custom_types import CustomType


class IPv4AddressType(CustomType[IPv4Address, str]):
    def serialize(self, value: IPv4Address) -> str:
        return str(value)

    def deserialize(self, value: str) -> IPv4Address:
        return IPv4Address(value)

    def get_json_schema(self):
        return {
            'type': 'string',
            'format': 'ipv4',
        }


def custom_type_resolver(t: type) -> Optional[CustomType]:
    if t is IPv4Address:
        return IPv4AddressType()
    return None


def test_custom_type():
    @dataclass
    class Data:
        ip: IPv4Address

    data = Data(ip=IPv4Address('1.1.1.1'))

    serializer = Serializer(Data, custom_type_resolver=custom_type_resolver)

    assert serializer.dump(data) == {'ip': '1.1.1.1'}
    assert serializer.load({'ip': '1.1.1.1'}) == data
    assert serializer.get_json_schema() == {
        '$ref': '#/components/schemas/tests.test_custom_types.test_custom_type.<locals>.Data',
        '$schema': 'https://json-schema.org/draft/2020-12/schema',
        'components': {
            'schemas': {
                'tests.test_custom_types.test_custom_type.<locals>.Data': {
                    'properties': {
                        'ip': {
                            'format': 'ipv4',
                            'type': 'string',
                        }
                    },
                    'required': ['ip'],
                    'type': 'object',
                }
            }
        },
    }


def test_custom_type__validation_error():
    @dataclass
    class Data:
        ip: IPv4Address

    serializer = Serializer(Data, custom_type_resolver=custom_type_resolver)
    with pytest.raises(SchemaValidationError) as exc_info:
        serializer.load({'ip': 'invalid'})

    assert exc_info.value.errors == [
        ErrorItem(message="AddressValueError: Expected 4 octets in 'invalid'", instance_path='ip')
    ]
    assert isinstance(exc_info.value.__cause__, AddressValueError)
