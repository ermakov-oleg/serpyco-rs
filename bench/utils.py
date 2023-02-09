from typing import Any, Callable


def repeat(func: Callable[[], Any], count: int = 10000) -> Callable[[], Any]:
    def inner():
        for i in range(count):
            func()

    return inner
