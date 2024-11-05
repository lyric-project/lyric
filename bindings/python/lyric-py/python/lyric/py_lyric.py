import asyncio
import json
import logging
import uuid
from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum
from typing import Any, Callable, Dict, List, Optional, Tuple, Union

from lyric.task import TaskInfo
from lyric_task import BaseTaskSpec, Language, LanguageType, WasmTaskSpec

from ._py_lyric import (
    PyDockerEnvironmentConfig,
    PyDriverConfig,
    PyEnvironmentConfig,
    PyLocalEnvironmentConfig,
    PyLyric,
    PyTaskHandle,
    PyTaskResourceConfig,
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


@dataclass
class WorkerInfo:
    """Basic information about a worker"""

    name: str  # Unique identifier for the worker, e.g. "python_worker"
    lang: Language  # Language type supported by the worker
    loader: "WorkerLoader"  # Worker's loader instance


class WorkerLoader(ABC):
    """Abstract base class for worker loaders"""

    async def load_worker(
        self,
        lyric: PyLyric,
        task_id: Optional[str] = None,
        task_name: Optional[str] = None,
        config: Optional[PyEnvironmentConfig] = None,
    ) -> PyTaskHandle:
        task_spec = self._task_spec()
        task_id = task_id or f"{str(uuid.uuid4())}"
        task_name = task_name or task_id
        task_info = TaskInfo.from_task(task_name, task_id, 3, task_spec)
        return await lyric.submit_task(task_info.to_core(), environment_config=config)

    @abstractmethod
    def _task_spec(self) -> BaseTaskSpec:
        """Get the task spec for the worker"""


class PythonWorkerLoader(WorkerLoader):
    def _task_spec(self) -> BaseTaskSpec:
        try:
            from lyric_py_worker import PythonWasmTaskSpec
        except ImportError:
            raise ImportError(
                "lyric_py_worker is not installed. Please install it with: pip install lyric-py-worker"
            )
        return PythonWasmTaskSpec()


class JavaScriptWorkerLoader(WorkerLoader):
    def _task_spec(self) -> BaseTaskSpec:
        try:
            from lyric_js_worker import JavaScriptWasmTaskSpec
        except ImportError:
            raise ImportError(
                "lyric_js_worker is not installed. Please install it with: pip install lyric-js-worker"
            )
        return JavaScriptWasmTaskSpec()


class RawWasmWorkerLoader(WorkerLoader):
    def __init__(self, wasm_path: str, lang: LanguageType = Language.PYTHON):
        self._wasm_path = wasm_path
        self._lang = lang

    def _task_spec(self) -> BaseTaskSpec:
        return WasmTaskSpec(self._wasm_path, self._lang)


@dataclass
class HandleItem:
    """Container for worker handle information and instance"""

    worker_info: WorkerInfo
    handle: PyTaskHandle
    environment_config: Optional[PyEnvironmentConfig] = None

    @property
    def uid(self) -> str:
        """Generate unique identifier for the handle"""
        config_id = (
            self.environment_config.env_id() if self.environment_config else "none"
        )
        return f"{self.worker_info.name}_{config_id}"


class HandleManager:
    """Manager for worker handles"""

    def __init__(self, lyric: PyLyric):
        self._lyric = lyric
        self._lock = asyncio.Lock()
        self._handles: Dict[str, HandleItem] = {}

    def get_worker_handles(self, worker_name: str) -> List[HandleItem]:
        """Get all handles for specified worker name"""
        return [h for h in self._handles.values() if h.worker_info.name == worker_name]

    def get_lang_handles(self, lang: Language) -> List[HandleItem]:
        """Get all handles for specified language"""
        return [h for h in self._handles.values() if h.worker_info.lang == lang]

    async def get_or_create_handler(
        self,
        worker_info: WorkerInfo,
        environment_config: Optional[PyEnvironmentConfig] = None,
        ignore_load_error: bool = False,
    ) -> Optional[HandleItem]:
        """Get existing handle or create new one"""
        async with self._lock:
            handle_item = HandleItem(worker_info, None, environment_config)
            uid = handle_item.uid

            if uid in self._handles:
                return self._handles[uid]

            try:
                handle = await worker_info.loader.load_worker(
                    self._lyric, config=environment_config
                )
                handle_item.handle = handle
                self._handles[uid] = handle_item
                return handle_item
            except Exception as e:
                if ignore_load_error:
                    logger.warning(f"Failed to load worker {worker_info.name}: {e}")
                    return None
                raise e


class ConfigurationManager:
    """Manager for environment configurations"""

    def __init__(
        self,
        default_local_env: Optional[PyLocalEnvironmentConfig] = None,
        default_docker_config: Optional[PyDockerEnvironmentConfig] = None,
    ):
        envs = {
            "LYRIC_CORE_LOG_ANSICOLOR": "false",
            "LYRIC_CORE_LOG_LEVEL": "ERROR",
        }
        self._default_local_config = default_local_env or PyLocalEnvironmentConfig(
            envs=envs
        )
        self._default_docker_config = (
            default_docker_config
            or PyDockerEnvironmentConfig(
                image="py-lyric-base-alpine:latest",
                mounts=[(self._get_base_lyric_dir(), "/app")],
                envs=envs,
            )
        )

    @staticmethod
    def _get_base_lyric_dir() -> str:
        from lyric import BASE_LYRIC_DIR

        return BASE_LYRIC_DIR

    def create_worker_config(self, worker_name: str) -> PyEnvironmentConfig:
        """Create default configuration for specified worker"""
        base_config = PyEnvironmentConfig(local=self._default_local_config)
        return base_config.clone_new(worker_name)

    def get_environment_config(
        self, exec_env: Optional[EXEC_ENV], worker_name: str
    ) -> PyEnvironmentConfig:
        """Get environment configuration"""
        env_config: Optional[PyEnvironmentConfig] = None
        if isinstance(exec_env, (ExecEnvType, str)):
            if exec_env == ExecEnvType.DOCKER:
                env_config = PyEnvironmentConfig(docker=self._default_docker_config)
            elif exec_env == ExecEnvType.LOCAL:
                env_config = PyEnvironmentConfig(local=self._default_local_config)
        elif isinstance(exec_env, PyEnvironmentConfig):
            env_config = exec_env
        elif isinstance(exec_env, PyLocalEnvironmentConfig):
            env_config = PyEnvironmentConfig(local=exec_env)
        elif isinstance(exec_env, PyDockerEnvironmentConfig):
            env_config = PyEnvironmentConfig(docker=exec_env)
        if not env_config:
            env_config = self.create_worker_config(worker_name)
        return env_config.clone_new(worker_name)


class Lyric:
    """Main Lyric class providing API interfaces"""

    DEFAULT_WORKERS = [
        WorkerInfo("python_worker", Language.PYTHON, PythonWorkerLoader()),
        WorkerInfo("javascript_worker", Language.JAVASCRIPT, JavaScriptWorkerLoader()),
    ]

    def __init__(
        self,
        pl: PyLyric,
        default_local_env: Optional[PyLocalEnvironmentConfig] = None,
        default_docker_config: Optional[PyDockerEnvironmentConfig] = None,
    ):
        self._pl = pl
        self._handle_manager = HandleManager(pl)
        self._config_manager = ConfigurationManager(
            default_local_env, default_docker_config
        )
        self._workers: Dict[str, WorkerInfo] = {w.name: w for w in self.DEFAULT_WORKERS}

    def start_driver(self):
        """Start the driver"""
        self._pl.start_driver(PyDriverConfig())

    def register_worker(
        self, name: str, lang: LanguageType, loader: WorkerLoader
    ) -> None:
        """Register a new worker

        Args:
            name: Unique name for the worker
            lang: Language type supported by the worker
            loader: Worker loader instance
        """
        if name in self._workers:
            raise ValueError(f"Worker '{name}' already exists")

        lang_instance = Language.parse(lang)
        worker_info = WorkerInfo(name, lang_instance, loader)
        self._workers[name] = worker_info

    async def load_worker(
        self, worker_name: str, exec_env: Optional[EXEC_ENV] = None
    ) -> None:
        """Load specified worker

        Args:
            worker_name: Name of the worker to load
            exec_env: Optional execution environment configuration
        """
        if worker_name not in self._workers:
            raise ValueError(f"Worker '{worker_name}' not found")

        worker_info = self._workers[worker_name]
        config = self._config_manager.get_environment_config(exec_env, worker_name)
        await self._handle_manager.get_or_create_handler(worker_info, config, True)

    async def load_default_workers(self) -> None:
        """Load all default workers"""
        for worker_info in self.DEFAULT_WORKERS:
            config = self._config_manager.create_worker_config(worker_info.name)
            await self._handle_manager.get_or_create_handler(worker_info, config, True)

    def get_worker_info(self, worker_name: str) -> Optional[WorkerInfo]:
        """Get worker information"""
        return self._workers.get(worker_name)

    def list_workers(self) -> List[WorkerInfo]:
        """List all registered workers"""
        return list(self._workers.values())

    async def _get_handler(
        self,
        lang: Language,
        worker_name: Optional[str] = None,
        exec_env: Optional[EXEC_ENV] = None,
    ) -> HandleItem:
        """Get handle that meets the conditions"""
        if worker_name:
            if worker_name not in self._workers:
                raise ValueError(f"Worker '{worker_name}' not found")
            worker_info = self._workers[worker_name]
            if worker_info.lang != lang:
                raise ValueError(
                    f"Worker '{worker_name}' does not support language {lang}"
                )
        else:
            # Use default worker
            worker_info = next(
                (w for w in self.DEFAULT_WORKERS if w.lang == lang), None
            )
            if not worker_info:
                raise ValueError(f"No default worker found for language {lang}")

        config = self._config_manager.get_environment_config(exec_env, worker_info.name)
        handle = await self._handle_manager.get_or_create_handler(worker_info, config)

        if not handle:
            raise RuntimeError(f"Failed to get handle for worker '{worker_info.name}'")
        return handle

    async def exec(
        self,
        code: str,
        lang: LanguageType = Language.PYTHON,
        worker_name: Optional[str] = None,
        decode: bool = True,
        exec_env: Optional[EXEC_ENV] = None,
        resources: Optional[PyTaskResourceConfig] = None,
    ) -> Union[Dict[str, Any], bytes]:
        """Execute code

        Args:
            code: Code to execute
            lang: Language type of the code
            worker_name: Optional worker name to use
            decode: Whether to decode the result
            exec_env: Optional execution environment configuration
            resources: Optional task resources
        """
        lang_instance = Language.parse(lang)
        handle = await self._get_handler(lang_instance, worker_name, exec_env)

        script_res = await handle.handle.exec(lang_instance.name, code, decode=decode, resources=resources)

        try:
            encoded = script_res.data
            if decode:
                json_str = bytes(encoded).decode("utf-8")
                return json.loads(json_str)
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
        lang: LanguageType = Language.PYTHON,
        worker_name: Optional[str] = None,
        exec_env: Optional[EXEC_ENV] = None,
        resources: Optional[PyTaskResourceConfig] = None,
    ) -> Union[Tuple[Dict[str, Any], Dict[str, Any]], Tuple[bytes, bytes]]:
        """Execute code with input

        Args:
            code: Code to execute
            input_bytes: Input data
            call_name: Function name to call
            encode: Whether to encode the input
            decode: Whether to decode the result
            lang: Language type of the code
            worker_name: Optional worker name to use
            exec_env: Optional execution environment configuration
            resources: Optional task resources
        """
        lang_instance = Language.parse(lang)
        handle = await self._get_handler(lang_instance, worker_name, exec_env)

        script_res = await handle.handle.exec1(
            lang_instance.name,
            code,
            call_name=call_name,
            input=input_bytes,
            encode=encode,
            decode=decode,
            resources=resources,
        )

        res_bytes = bytes(script_res[0].data)
        output_bytes = bytes(script_res[1].data)

        try:
            if decode:
                return (
                    json.loads(res_bytes.decode("utf-8")),
                    json.loads(output_bytes.decode("utf-8")),
                )
            return res_bytes, output_bytes
        except Exception as e:
            logger.error(f"Failed to parse the result: {e}")
            raise e

    def stop(self):
        """Stop Lyric instance"""
        self._pl.stop()

    def join(self):
        """Wait for all tasks to complete"""
        self._pl.join()
