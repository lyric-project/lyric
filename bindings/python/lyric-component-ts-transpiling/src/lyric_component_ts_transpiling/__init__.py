"""Wrapper for the TypeScript transpiling WebAssembly module.

May be we should find a better way to install the wasm file, but for now we just pypi 
install the package and the wasm file is included in the package.
"""

from importlib.resources import files
from lyric_task import WasmTaskSpec, Language

def get_wasm_path():
    return files('lyric_component_ts_transpiling').joinpath('component_ts_transpiling.wasm')


class TypeScriptWasmTaskSpec(WasmTaskSpec):
    def __init__(self):
        super().__init__(str(get_wasm_path()), Language.WASI)