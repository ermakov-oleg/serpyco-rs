"""PGO training run (`pgo-command` in pyproject.toml).

A script rather than an inline pytest command: maturin runs the command through
`cmd /C` on Windows, which mangles the quotes a `-k "..."` expression needs.
"""

import sys

import pytest


# The codec (bytes) benches must stay listed: code left out of the profile is
# compiled as cold, worth -21%..-35% on the codec path.
BENCHES = [
    'bench/test_encoders.py',
    'bench/test_codec_encoders.py',
    'bench/test_flatten.py',
    'bench/test_full.py',
    'bench/compare/test_github_issue.py',
    'bench/compare/test_github_issue_bytes.py',
]

if __name__ == '__main__':
    sys.exit(
        pytest.main(
            [
                *BENCHES,
                '-k',
                'not mashumaro and not msgspec and not orjson and not ormsgpack',
                '--benchmark-min-time=0.2',
                '--benchmark-max-time=0.4',
            ]
        )
    )
