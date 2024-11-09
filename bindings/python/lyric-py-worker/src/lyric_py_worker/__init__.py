from importlib.resources import files

from lyric_task import Language, WasmTaskSpec


def get_wasm_path():
    return files("lyric_py_worker").joinpath("python_worker.wasm")


class PythonWasmTaskSpec(WasmTaskSpec):
    def __init__(self):
        super().__init__(str(get_wasm_path()), Language.PYTHON)
