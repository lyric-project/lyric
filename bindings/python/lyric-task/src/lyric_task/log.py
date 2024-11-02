"""Logging utilities for lyric-task."""

import sys
from contextlib import contextmanager
from io import StringIO
from typing import Any, Iterator


class TeeIO(StringIO):
    def __init__(self, original_stream, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.original_stream = original_stream

    def write(self, s):
        super().write(s)
        self.original_stream.write(s)

    def flush(self):
        super().flush()
        self.original_stream.flush()


class IOCapture:
    def __init__(self):
        self.stdout = None
        self.stderr = None
        self.original_stdout = sys.stdout
        self.original_stderr = sys.stderr

    def __enter__(self):
        self.stdout = TeeIO(self.original_stdout)
        self.stderr = TeeIO(self.original_stderr)
        sys.stdout = self.stdout
        sys.stderr = self.stderr
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        sys.stdout = self.original_stdout
        sys.stderr = self.original_stderr
        self.stdout = None
        self.stderr = None

    def get_output(self):
        if self.stdout is None or self.stderr is None:
            return "", ""
        stdout = self.stdout.getvalue()
        stderr = self.stderr.getvalue()
        self.clear()
        return stdout, stderr

    def clear(self):
        if self.stdout is not None:
            self.stdout.truncate(0)
            self.stdout.seek(0)
        if self.stderr is not None:
            self.stderr.truncate(0)
            self.stderr.seek(0)

    @contextmanager
    def direct_output(self):
        old_stdout, old_stderr = sys.stdout, sys.stderr
        try:
            sys.stdout, sys.stderr = self.original_stdout, self.original_stderr
            yield
        finally:
            sys.stdout, sys.stderr = old_stdout, old_stderr


def capture_iterator_output(iterator: Iterator[Any], map_fn) -> Iterator[Any]:
    with IOCapture() as capture:
        for item in iterator:
            stdout, stderr = capture.get_output()
            yield map_fn(item, stdout, stderr)
