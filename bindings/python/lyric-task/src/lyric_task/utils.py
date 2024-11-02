import asyncio
import inspect
import logging
from typing import cast


def is_async_iterator(obj):
    return (
        inspect.isasyncgen(obj)  # Check if it is an async generator
        or (
            hasattr(obj, "__aiter__") and inspect.iscoroutinefunction(obj.__aiter__)
        )  # Check if it has an async __aiter__ method
        or (
            hasattr(obj, "__anext__") and inspect.iscoroutinefunction(obj.__anext__)
        )  # Check if it has an async __anext__ method
    )


def is_iterator(obj):
    return (
        hasattr(obj, "__iter__")
        and callable(getattr(obj, "__iter__", None))
        and hasattr(obj, "__next__")
        and callable(getattr(obj, "__next__", None))
    )


def get_or_create_event_loop() -> asyncio.BaseEventLoop:
    loop = None
    try:
        loop = asyncio.get_event_loop()
        assert loop is not None
        return cast(asyncio.BaseEventLoop, loop)
    except RuntimeError as e:
        if not "no running event loop" in str(e) and not "no current event loop" in str(
            e
        ):
            raise e
        logging.warning("Cant not get running event loop, create new event loop now")
    return cast(asyncio.BaseEventLoop, asyncio.get_event_loop_policy().new_event_loop())


class AsyncToSyncIterator:
    def __init__(self, async_iterable, loop: asyncio.BaseEventLoop):
        self.async_iterable = async_iterable
        self.async_iterator = None
        self._loop = loop

    def __iter__(self):
        self.async_iterator = self.async_iterable.__aiter__()
        return self

    def __next__(self):
        if self.async_iterator is None:
            raise StopIteration

        try:
            return self._loop.run_until_complete(self.async_iterator.__anext__())
        except StopAsyncIteration:
            raise StopIteration
