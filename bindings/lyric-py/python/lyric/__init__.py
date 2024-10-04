from __future__ import annotations

import os

from ._py_lyric import *
from .task import TaskInfo
from .py_lyric import Lyric

BASE_LYRIC_DIR = os.path.dirname(os.path.abspath(__file__))
DEFAULT_WORKER_FILE = "default_python_worker.py"
DEFAULT_WORKER_PATH = os.path.join(BASE_LYRIC_DIR, DEFAULT_WORKER_FILE)

__doc__ = _py_lyric.__doc__
if hasattr(_py_lyric, "__all__"):
    __all__ = _py_lyric.__all__
