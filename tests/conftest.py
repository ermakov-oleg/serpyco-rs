import sys

if sys.version_info[:2] < (3, 11):
    collect_ignore_glob = ["*_py311.py"]
