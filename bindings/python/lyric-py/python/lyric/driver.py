import logging
import sys
from dataclasses import dataclass
from typing import Dict, Optional

from lyric_task import Language, LanguageType

from ._py_lyric import PyConfig, PyLocalEnvironmentConfig, PyLyric, PyTaskResourceConfig
from .config import DEFAULT_WORKER_PATH
from .py_lyric import Lyric

logger = logging.getLogger(__name__)


@dataclass
class CodeResult:
    """Container for code execution results

    Attributes:
        exit_code: Exit status of the code execution (0 for success)
        stdout: Standard output from the execution
        stderr: Standard error output from the execution
        output: Optional dictionary containing additional execution data
    """

    exit_code: int
    stdout: str
    stderr: str
    output: Optional[Dict] = None

    @property
    def logs(self) -> str:
        """Combine stdout and stderr into a single log string

        Returns:
            Combined log output with stderr appended after stdout if present
        """
        logs = self.stdout
        if self.stderr:
            logs += f"\n{self.stderr}"
        return logs


class DefaultLyricDriver:
    """A simplified driver for the Lyric execution environment

    This class provides a higher-level interface for code execution across different
    programming languages, handling worker management and environment configuration.

    Attributes:
        py_config: Configuration for the Python Lyric driver
        _lyric: Instance of the core Lyric execution engine
    """

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 15670,
        maximum_workers: int = 20,
        minimum_workers: int = 1,
        log_level: str = "ERROR",
        eventloop_worker_threads: int = 10,
    ):
        """Initialize the DefaultLyricDriver

        Args:
            host: Host address for the driver (default: "127.0.0.1")
            port: Port number for the driver (default: 15670)
            maximum_workers: Maximum number of concurrent worker processes (default: 20)
            minimum_workers: Minimum number of worker processes to maintain (default: 1)
            log_level: Logging level for the driver (default: "ERROR")
            eventloop_worker_threads: Number of worker threads for the event loop (default: 10)
        """
        # Handle localhost alias
        if host == "localhost":
            host = "127.0.0.1"

        # Configure host-specific settings
        if host == "127.0.0.1":
            public_host = "127.0.0.1"
            worker_host = " --host 127.0.0.1"
            driver_public_host = "--public-host 127.0.0.1"
        else:
            public_host = None
            worker_host = ""
            driver_public_host = ""

        # Construct worker command with appropriate host settings
        default_worker = (
            f"{sys.executable} {DEFAULT_WORKER_PATH} {worker_host} {driver_public_host}"
        )

        # Initialize driver configuration
        self.py_config = PyConfig(
            host=host,
            port=port,
            is_driver=True,
            public_host=public_host,
            worker_port_start=15671,
            worker_port_end=16670,
            maximum_workers=maximum_workers,
            minimum_workers=minimum_workers,
            worker_start_commands={
                "PYTHON": default_worker,
                "RUST": default_worker,
                "JAVASCRIPT": default_worker,
            },
            eventloop_worker_threads=eventloop_worker_threads,
            log_level=log_level,
        )

        # Initialize default environment configuration
        default_local_env = PyLocalEnvironmentConfig(
            envs={
                "LYRIC_CORE_LOG_ANSICOLOR": "false",
                "LYRIC_CORE_LOG_LEVEL": log_level,
            }
        )

        # Create Lyric instance
        self._lyric = Lyric(
            PyLyric(self.py_config), default_local_env=default_local_env
        )

    @property
    def lyric(self) -> Lyric:
        """Get the underlying Lyric instance."""
        return self._lyric

    def start(self):
        """Start the Lyric driver."""
        self._lyric.start_driver()

    async def exec(
        self,
        code: str,
        lang: LanguageType = Language.PYTHON,
        worker_name: Optional[str] = None,
        decode: bool = True,
        resources: Optional[PyTaskResourceConfig] = None,
    ) -> CodeResult:
        """Execute code in the specified language

        Args:
            code: Source code to execute
            lang: Programming language of the code (default: Python)
            worker_name: Optional name of specific worker to use
            decode: Whether to decode the execution result (default: True)
            resources: Optional resource configuration for the task
        Returns:
            CodeResult containing execution status and output
        """
        try:
            res = await self._lyric.exec(
                code, lang=lang, worker_name=worker_name, decode=decode, resources=resources
            )
            stdout = res.get("stdout", "")
            stderr = res.get("stderr", "")
            exit_code = res.get("exit_code", 1)
        except Exception as e:
            stdout = ""
            stderr = str(e)
            exit_code = 1
        return CodeResult(exit_code=exit_code, stdout=stdout, stderr=stderr)

    async def exec1(
        self,
        code: str,
        input_bytes: bytes,
        call_name: str,
        encode: bool = True,
        decode: bool = True,
        lang: LanguageType = Language.PYTHON,
        worker_name: Optional[str] = None,
        resources: Optional[PyTaskResourceConfig] = None,
    ) -> CodeResult:
        """Execute code with input data and a specific function call

        Args:
            code: Source code to execute
            input_bytes: Input data as bytes
            call_name: Name of the function to call in the code
            encode: Whether to encode the input (default: True)
            decode: Whether to decode the result (default: True)
            lang: Programming language of the code (default: Python)
            worker_name: Optional name of specific worker to use
            resources: Optional resource configuration for the task

        Returns:
            CodeResult containing execution status, output and additional data
        """
        output = None
        try:
            res, output = await self._lyric.exec1(
                code,
                input_bytes,
                call_name,
                encode=encode,
                decode=decode,
                lang=lang,
                worker_name=worker_name,
                resources=resources,
            )
            stdout = res.get("stdout", "")
            stderr = res.get("stderr", "")
            exit_code = res.get("exit_code", 1)
        except Exception as e:
            stdout = ""
            stderr = str(e)
            exit_code = 1
        return CodeResult(
            exit_code=exit_code, stdout=stdout, stderr=stderr, output=output
        )

    def stop(self):
        """Stop the Lyric driver and cleanup resources."""
        self._lyric.stop()
