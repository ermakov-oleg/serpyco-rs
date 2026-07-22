import pytest

import serpyco_rs


def test_decode_error_exported():
    assert issubclass(serpyco_rs.DecodeError, ValueError)
