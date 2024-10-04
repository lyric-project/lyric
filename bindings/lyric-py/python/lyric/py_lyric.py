import asyncio
import uuid
from enum import Enum
from typing import Union, Optional

from ._py_lyric import (
    PyLyric,
    PyEnvironmentConfig,
    PyLocalEnvironmentConfig,
    PyDockerEnvironmentConfig,
    PyDriverConfig,
)
from lyric.task import (
    NormalCodeTask,
    TaskInfo,
    unwrapper_task_output,
)

class ExecEnvType(str, Enum):
    LOCAL = "local"
    DOCKER = "docker"

EXEC_ENV = Union[str, ExecEnvType, PyLocalEnvironmentConfig, PyDockerEnvironmentConfig, PyEnvironmentConfig]

class Lyric:
    def __init__(self, pl: PyLyric):
        from lyric import BASE_LYRIC_DIR

        self._pl = pl
        self._default_local_env = (
            PyLocalEnvironmentConfig(envs={"LYRIC_CORE_LOG_ANSICOLOR": "false"}),
        )
        self._default_docker_env = PyDockerEnvironmentConfig(
            image="py-lyric-base-alpine:latest", mounts=[(BASE_LYRIC_DIR, "/app")]
        )

    def start_driver(self):
        self._pl.start_driver(PyDriverConfig())

    def stop(self):
        self._pl.stop()

    def join(self):
        self._pl.join()

    def _gen_task_id(self) -> str:
        return str(uuid.uuid4())

    async def submit_python_code(
        self,
        code: str,
        task_name: str = "python_code_task",
        task_id: str | None = None,
        exec_env: Optional[EXEC_ENV] = None,
        callback=None,
    ):
        task = NormalCodeTask(code)
        task_id = task_id or self._gen_task_id()
        task_info = TaskInfo.from_task(task_name, task_id, 0, task)
        environment_config = self._get_environment_config(exec_env)

        result_queue = asyncio.Queue()
        loop = asyncio.get_event_loop()
        def wrapped_callback(task_state_info):
            try:
                if hasattr(task_state_info, "__next__"):
                    for item in task_state_info:
                        out = unwrapper_task_output(item)
                        loop.call_soon_threadsafe(result_queue.put_nowait, out)
                        if callback:
                            callback(out)
                else:
                    out = unwrapper_task_output(task_state_info)
                    loop.call_soon_threadsafe(result_queue.put_nowait, out)
                    if callback:
                        callback(out)
            except Exception as e:
                loop.call_soon_threadsafe(result_queue.put_nowait, e)
            finally:
                loop.call_soon_threadsafe(result_queue.put_nowait, None)

        await self._pl.submit_task_async(
            task_info.to_core(),
            wrapped_callback,
            environment_config=environment_config
        )

        results = []
        while True:
            item = await result_queue.get()
            if item is None:
                break
            if isinstance(item, Exception):
                raise item
            results.append(item)

        return results if len(results) > 1 else results[0] if results else None

    def _get_environment_config(self, exec_env: Optional[EXEC_ENV] = None) -> Optional[PyEnvironmentConfig]:
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
        return None