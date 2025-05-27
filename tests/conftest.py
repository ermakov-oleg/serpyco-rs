import sys

import pytest
from serpyco_rs._describe import _NAME_CACHE, _generate_name


collect_ignore_glob = []

if sys.version_info[:2] < (3, 11):
    collect_ignore_glob += ['*_py311.py']

if sys.version_info[:2] < (3, 12):
    collect_ignore_glob += ['*_py312.py']


@pytest.fixture(autouse=True)
def _clear_name_cache():
    # Clear the name cache before each test to avoid side effects between tests
    _NAME_CACHE.clear()
    _generate_name.cache_clear()
