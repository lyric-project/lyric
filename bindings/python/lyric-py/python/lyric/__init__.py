"""Lyric Python binding.

Lyric: A Rust-powered secure sandbox for multi-language code execution, leveraging WebAssembly to provide
high-performance runtime isolation for AI applications.

This module provides a Python binding for Lyric, which allows you to execute code in multiple languages

Getting started:

1. Install lyric-py:

    .. code-block:: bash

        pip install lyric-py

2. Install default Python webassembly worker:

    .. code-block:: bash

        pip install lyric-py-worker

3. Install default JavaScript webassembly worker:

    .. code-block:: bash

        pip install lyric-js-worker

Examples:
    .. code-block:: python

        import asyncio
        from lyric import DefaultLyricDriver

        lcd = DefaultLyricDriver(host="localhost", log_level="ERROR")
        lcd.start()

        # Load workers(default: Python, JavaScript)
        asyncio.run(lcd.lyric.load_default_workers())

        # Execute Python code
        python_code = \"""
        def add(a, b):
            return a + b
        result = add(1, 2)
        print(result)
        \"""

        py_res = asyncio.run(lcd.exec(python_code, "python"))
        print(py_res)

        # Execute JavaScript code
        js_code = \"""
        console.log('Hello from JavaScript!');
        \"""

        js_res = asyncio.run(lcd.exec(js_code, "javascript"))
        print(js_res)

        # Stop the driver
        lcd.stop()

"""

from ._py_lyric import *
from .config import BASE_LYRIC_DIR, DEFAULT_WORKER_FILE, DEFAULT_WORKER_PATH
from .driver import CodeResult, DefaultLyricDriver
from .py_lyric import Lyric
from .task import TaskInfo

__doc__ = _py_lyric.__doc__
if hasattr(_py_lyric, "__all__"):
    __all__ = _py_lyric.__all__
else:
    __all__ = []

__ALL__ = __all__
__ALL__.extend(
    [
        "BASE_LYRIC_DIR",
        "DEFAULT_WORKER_FILE",
        "DEFAULT_WORKER_PATH",
        "Lyric",
        "TaskInfo",
        "DefaultLyricDriver",
        "CodeResult",
    ]
)
