import asyncio
import json
import logging
import uuid
from dataclasses import dataclass
from enum import Enum
from typing import Callable, Dict, List, Optional, Tuple, Union

from lyric.task import TaskInfo
from lyric_task import Language, LanguageType

from ._py_lyric import (
    PyDockerEnvironmentConfig,
    PyDriverConfig,
    PyEnvironmentConfig,
    PyLocalEnvironmentConfig,
    PyLyric,
    PyTaskHandle,
)

logger = logging.getLogger(__name__)


class ExecEnvType(str, Enum):
    LOCAL = "local"
    DOCKER = "docker"


EXEC_ENV = Union[
    str,
    ExecEnvType,
    PyLocalEnvironmentConfig,
    PyDockerEnvironmentConfig,
    PyEnvironmentConfig,
]


def _gen_task_id() -> str:
    return str(uuid.uuid4())


class Lyric:
    def __init__(self, pl: PyLyric):
        from lyric import BASE_LYRIC_DIR

        self._pl = pl
        self._hm = HandleManager(pl)
        self._default_local_env = PyLocalEnvironmentConfig(
            envs={"LYRIC_CORE_LOG_ANSICOLOR": "false"}
        )
        self._default_docker_env = PyDockerEnvironmentConfig(
            image="py-lyric-base-alpine:latest", mounts=[(BASE_LYRIC_DIR, "/app")]
        )

    def start_driver(self):
        self._pl.start_driver(PyDriverConfig())

    async def load_workers(self, languages: List[LanguageType] = None):
        languages = languages or [Language.PYTHON, Language.JAVA_SCRIPT]
        languages = [Language.parse(lang) for lang in languages]
        if Language.PYTHON in languages:
            await self._hm.get_handler(
                Language.PYTHON,
                _load_python_worker,
                self._default_env(Language.PYTHON),
                ignore_load_error=True,
            )
        if Language.JAVA_SCRIPT in languages:
            await self._hm.get_handler(
                Language.JAVA_SCRIPT,
                _load_javascript_worker,
                self._default_env(Language.JAVA_SCRIPT),
                ignore_load_error=True,
            )

    def _default_env(self, language: LanguageType) -> PyEnvironmentConfig:
        language = Language.parse(language)
        if language == Language.PYTHON:
            return PyEnvironmentConfig(local=self._default_local_env).clone_new(
                "python_worker"
            )
        elif language == Language.JAVA_SCRIPT:
            return PyEnvironmentConfig(local=self._default_local_env).clone_new(
                "javascript_worker"
            )
        return PyEnvironmentConfig(local=self._default_local_env)

    def stop(self):
        self._pl.stop()

    def join(self):
        self._pl.join()

    async def exec(
        self,
        code: str,
        language: LanguageType = Language.PYTHON,
        exec_env: Optional[EXEC_ENV] = None,
        decode: bool = True,
    ) -> Union[Dict[str, any], bytes]:
        lang = Language.parse(language)
        handle = await self._hm.get_handler(
            lang, _load_python_worker, self._get_environment_config(exec_env, lang)
        )
        if handle is None:
            raise RuntimeError(
                f"Failed to run code for {lang}, because the worker is not available"
            )
        script_res = await handle.handle.exec(lang.name, code, decode=decode)
        try:
            encoded = script_res.data
            if decode:
                # The result has been decoded in the worker, just parse it to dict
                json_str = bytes(encoded).decode("utf-8")
                parsed_data = json.loads(json_str)
                return parsed_data
            # Return the encoded bytes
            return encoded
        except Exception as e:
            logger.error(f"Failed to parse the result: {e}")
            raise e

    async def exec1(
        self,
        code: str,
        input_bytes: bytes,
        call_name: str,
        encode: bool = True,
        decode: bool = True,
        language: LanguageType = Language.PYTHON,
        exec_env: Optional[EXEC_ENV] = None,
    ) -> Union[Tuple[Dict[str, any], Dict[str, any]], Tuple[bytes, bytes]]:
        lang = Language.parse(language)
        handle = await self._hm.get_handler(
            lang, _load_python_worker, self._get_environment_config(exec_env, lang)
        )
        if handle is None:
            raise RuntimeError(
                f"Failed to run code for {lang}, because the worker is not available"
            )
        script_res = await handle.handle.exec1(
            lang.name,
            code,
            call_name=call_name,
            input=input_bytes,
            encode=encode,
            decode=decode,
        )
        res_bytes = bytes(script_res[0].data)
        output_bytes = bytes(script_res[1].data)
        try:
            if decode:
                # The result has been decoded in the worker, just parse it to dict
                return json.loads(res_bytes.decode("utf-8")), json.loads(
                    output_bytes.decode("utf-8")
                )
            # Return the encoded bytes
            return res_bytes, output_bytes
        except Exception as e:
            logger.error(f"Failed to parse the result: {e}")
            raise e

    def _get_environment_config(
        self, exec_env: Optional[EXEC_ENV] = None, lang: LanguageType = Language.PYTHON
    ) -> Optional[PyEnvironmentConfig]:
        if isinstance(exec_env, (ExecEnvType, str)):
            if exec_env == ExecEnvType.DOCKER:
                return PyEnvironmentConfig(docker=self._default_docker_env)
            elif exec_env == ExecEnvType.LOCAL:
                return PyEnvironmentConfig(local=self._default_local_env)
        elif isinstance(exec_env, PyEnvironmentConfig):
            return exec_env
        elif isinstance(exec_env, PyLocalEnvironmentConfig):
            return PyEnvironmentConfig(local=exec_env)
        elif isinstance(exec_env, PyDockerEnvironmentConfig):
            return PyEnvironmentConfig(docker=exec_env)
        return self._default_env(lang)


@dataclass
class HandleItem:
    lang: Language
    handle: PyTaskHandle
    environment_config: Optional[PyEnvironmentConfig] = None

    @classmethod
    def get_uid(
        cls, lang: Language, environment_config: Optional[PyEnvironmentConfig]
    ) -> str:
        if environment_config is None:
            return f"{lang.value}_none"
        return f"{lang.value}_{environment_config.env_id()}"

    @property
    def uid(self) -> str:
        return self.get_uid(self.lang, self.environment_config)


class HandleManager:
    def __init__(self, lyric: PyLyric):
        self._lyric = lyric
        self._lock = asyncio.Lock()
        self._handles: Dict[str, HandleItem] = {}

    async def get_handler(
        self,
        lang: Language,
        load_func: Callable[[PyLyric, Optional[PyEnvironmentConfig]], PyTaskHandle],
        environment_config: Optional[PyEnvironmentConfig] = None,
        ignore_load_error: bool = False,
    ) -> Optional[HandleItem]:
        lang = Language.parse(lang)
        uid = HandleItem.get_uid(lang, environment_config)
        async with self._lock:
            if uid in self._handles:
                return self._handles[uid]
            try:
                handle = await load_func(self._lyric, environment_config)
                item = HandleItem(lang, handle, environment_config)
                self._handles[uid] = item
                return item
            except Exception as e:
                if ignore_load_error:
                    logger.warning(f"Failed to load worker for {lang}: {e}")
                    return None
                raise e


async def _load_python_worker(
    lyric: PyLyric, environment_config: PyEnvironmentConfig
) -> PyTaskHandle:
    try:
        from lyric_py_worker import PythonWasmTaskSpec
    except ImportError:
        raise ImportError(
            "lyric_py_worker is not installed. Please install it to use Python worker, you can install it by running "
            "`pip install lyric-py-worker`"
        )

    py_task = PythonWasmTaskSpec()
    task_id = "py_" + _gen_task_id()
    py_task_info = TaskInfo.from_task("python_worker_task", task_id, 3, py_task)
    py_handler = await lyric.submit_task(
        py_task_info.to_core(), environment_config=environment_config
    )
    return py_handler


async def _load_javascript_worker(
    lyric: PyLyric, environment_config: PyEnvironmentConfig
) -> PyTaskHandle:
    try:
        from lyric_js_worker import JavaScriptWasmTaskSpec
    except ImportError:
        raise ImportError(
            "lyric_js_worker is not installed. Please install it to use JavaScript worker, you can install it by running `pip install lyric-js-worker`"
        )

    js_task = JavaScriptWasmTaskSpec()
    task_id = "js_" + _gen_task_id()
    js_task_info = TaskInfo.from_task("javascript_worker_task", task_id, 3, js_task)
    js_handler = await lyric.submit_task(
        js_task_info.to_core(), environment_config=environment_config
    )
    return js_handler
