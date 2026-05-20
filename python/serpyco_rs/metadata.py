from collections.abc import Callable, Mapping
from dataclasses import dataclass
from enum import Enum
from typing import Any, TypeVar

from ._type_info import CustomEncoder


@dataclass(frozen=True)
class Min:
    """Lower bound for numeric fields. Use with ``Annotated[int, Min(0)]``.

    ``inclusive=False`` switches the comparison to strict (``>``).
    """

    value: int | float
    inclusive: bool = True


@dataclass(frozen=True)
class Max:
    """Upper bound for numeric fields. Use with ``Annotated[int, Max(100)]``.

    ``inclusive=False`` switches the comparison to strict (``<``).
    """

    value: int | float
    inclusive: bool = True


@dataclass(frozen=True)
class MinLength:
    """Minimum length for strings/sequences. Use with ``Annotated[str, MinLength(1)]``."""

    value: int


@dataclass(frozen=True)
class MaxLength:
    """Maximum length for strings/sequences. Use with ``Annotated[str, MaxLength(64)]``."""

    value: int


@dataclass(frozen=True)
class Discriminator:
    """Tag field used to dispatch a tagged ``Union`` during deserialization.

    Attach to a ``Union`` type via ``Annotated[Union[A, B], Discriminator('kind')]``;
    each variant must expose the discriminator field with a unique literal value.
    """

    name: str


@dataclass(frozen=True)
class Alias:
    """External field name used during dump/load.

    Use with ``Annotated[T, Alias('externalName')]`` when the Python attribute
    name differs from the wire-format key.
    """

    value: str


class Format(Enum):
    no_format = 'no_format'
    camel_case = 'camel_case'


@dataclass(frozen=True)
class FieldFormat:
    """Per-field key formatting. Prefer the ``CamelCase`` / ``NoFormat`` singletons
    over constructing this directly.
    """

    format: Format


CamelCase: FieldFormat = FieldFormat(Format.camel_case)
NoFormat: FieldFormat = FieldFormat(Format.no_format)


@dataclass(frozen=True)
class NoneFormat:
    omit: bool


KeepNone: NoneFormat = NoneFormat(False)
OmitNone: NoneFormat = NoneFormat(True)


@dataclass(frozen=True)
class NoneAsDefaultForOptional:
    use: bool


ForceDefaultForOptional: NoneAsDefaultForOptional = NoneAsDefaultForOptional(True)
KeepDefaultForOptional: NoneAsDefaultForOptional = NoneAsDefaultForOptional(False)


_I = TypeVar('_I')
_O = TypeVar('_O')


def serialize_with(func: Callable[[_I], _O]) -> CustomEncoder[_I, _O]:
    return CustomEncoder[_I, _O](serialize=func)


def deserialize_with(func: Callable[[_O], _I]) -> CustomEncoder[_I, _O]:
    return CustomEncoder[_I, _O](deserialize=func)


class JsonSchemaExtension:
    """Arbitrary JSON Schema fragment merged into the generated schema for a field.

    Use with ``Annotated[T, JsonSchemaExtension({'description': '...', 'examples': [...]})]``.
    Does not affect runtime serialization — only the output of ``get_json_schema``.
    """

    def __init__(self, schema: Mapping[str, Any]) -> None:
        self._pairs = tuple(sorted(schema.items()))
        self.schema: Mapping[str, Any] = dict(self._pairs)

    def __hash__(self) -> int:
        return hash(self._pairs)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, JsonSchemaExtension):
            return self.schema == other.schema
        return NotImplemented

    def __str__(self) -> str:
        return f'JsonSchemaExtension(schema={self.schema})'


@dataclass(frozen=True)
class _Flatten:
    """Flatten the fields of a nested structure into the parent structure.

    Similar to serde's flatten attribute, this allows inlining fields from
    a nested dataclass or collecting additional fields in a dict.

    Examples:
        # Flatten a nested dataclass
        @dataclass
        class Person:
            name: str
            address: Annotated[Address, Flatten]

        # Collect additional fields in a dict
        @dataclass
        class FlexibleData:
            id: str
            extra: Annotated[dict[str, Any], Flatten]
    """


Flatten: _Flatten = _Flatten()
