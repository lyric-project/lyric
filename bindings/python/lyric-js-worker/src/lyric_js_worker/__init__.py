from importlib.resources import files
from lyric_task import WasmTaskSpec, Language

def get_wasm_path():
    return files('lyric_js_worker').joinpath('javascript_worker.wasm')


class JavaScriptWasmTaskSpec(WasmTaskSpec):
    def __init__(self):
        super().__init__(str(get_wasm_path()), Language.JAVASCRIPT)