from ._impl import ValidationError

__all__ = ["ValidationError", "SchemaValidationError"]


class SchemaValidationError(ValidationError):
    pass
