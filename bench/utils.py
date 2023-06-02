from typing import Any, Callable


def repeat(func: Callable[[], Any], count: int = 1000) -> Callable[[], Any]:
    def inner():
        for _i in range(count):
            func()

    return inner
