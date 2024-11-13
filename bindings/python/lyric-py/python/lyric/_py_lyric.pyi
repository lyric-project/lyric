from typing import Any, Callable, Generic, List, Literal, Tuple, TypeVar, Union, final

from typing_extensions import Self

__version__: str

from typing import Dict, Optional, final

@final
class PyConfig:
    """Configuration for both driver and worker nodes.

    This class contains settings that how to configure the Lyric node.

    Attributes:
        host: The host address to bind to, or None for auto-assigned, it can be "localhost", "0.0.0.0" or "127.0.0.1".
        port: The port to listen on
        public_host: The public host address for external access
        is_driver: Whether this node is a driver node
        worker_port_start: Starting port number for worker allocation
        worker_port_end: Ending port number for worker allocation
        maximum_workers: Maximum number of workers allowed
        minimum_workers: Minimum number of workers to maintain
        worker_start_commands: Commands to start different types of workers
        node_id: Unique identifier for this node
        eventloop_worker_threads: Number of worker threads for the event loop
        log_level: Logging level for the system
    """

    host: Optional[str]
    port: Optional[int]
    public_host: Optional[str]
    is_driver: bool
    worker_port_start: int
    worker_port_end: int
    maximum_workers: int
    minimum_workers: int
    worker_start_commands: Dict[str, str]
    node_id: Optional[str]
    eventloop_worker_threads: Optional[int]
    log_level: Optional[str]

    def __init__(
        self,
        is_driver: bool,
        host: Optional[str] = None,
        port: Optional[int] = None,
        public_host: Optional[str] = None,
        worker_port_start: Optional[int] = None,
        worker_port_end: Optional[int] = None,
        maximum_workers: Optional[int] = None,
        minimum_workers: Optional[int] = None,
        worker_start_commands: Optional[Dict[str, str]] = None,
        node_id: Optional[str] = None,
        eventloop_worker_threads: Optional[int] = None,
        log_level: Optional[str] = None,
    ) -> None: ...

@final
class PyWorkerConfig:
    """Configuration specific to worker nodes.

    This class contains settings needed for a worker node to connect to its driver
    and configure its network behavior.

    Attributes:
        driver_address: The address of the driver node to connect to
        network_mode: Optional network mode configuration
    """

    driver_address: str
    network_mode: Optional[str]

    def __init__(
        self, driver_address: str, network_mode: Optional[str] = None
    ) -> None: ...

@final
class PyDriverConfig:
    """Configuration specific to driver nodes.

    Currently this is an empty configuration class that serves as a placeholder
    for future driver-specific settings.
    """

    def __init__(self) -> None: ...

@final
class PyDataObject:
    """A data object containing binary data and metadata.

    Attributes:
        object_id: Unique identifier for the data object
        format: Integer representing the data format
        data: Binary data as bytes
    """

    object_id: str
    format: int
    data: bytes

    def __init__(self, object_id: str, format: int, data: bytes) -> None: ...
    def __str__(self) -> str: ...

@final
class PyTaskOutputObject:
    """Output from a task execution including data and logs.

    Attributes:
        data: The output data object
        stderr: Standard error output
        stdout: Standard output
    """

    data: PyDataObject
    stderr: str
    stdout: str

    def __init__(self, data: PyDataObject, stderr: str, stdout: str) -> None: ...
    def __str__(self) -> str: ...

@final
class PyStreamDataObject:
    """A stream of data objects."""

    def __init__(self) -> None: ...

@final
class PyStreamDataObjectIter:
    """Iterator for streaming data objects.

    Provides iteration over a stream of task state information.
    """

    def __iter__(self) -> Self: ...
    def __next__(self) -> Optional[PyTaskStateInfo]: ...

@final
class PyExecutionUnit:
    """Unit of execution for a task.

    Attributes:
        unit_id: Unique identifier for the execution unit
        language: Programming language identifier
        code: Optional code to execute
    """

    unit_id: str
    language: int
    code: Optional[PyDataObject]

    def __init__(
        self, unit_id: str, language: int, code: Optional[PyDataObject] = None
    ) -> None: ...
    def __str__(self) -> str: ...

@final
class PyTaskInfo:
    """Information about a task to be submitted.

    Attributes:
        task_id: Unique identifier for the task
        name: Task name
        language: Programming language identifier
        exec_mode: Execution mode identifier
        exec_unit: Optional execution unit
        input: Optional input data
        stream_input: Optional streaming input
        streaming_result: Whether the task produces streaming results
    """

    task_id: str
    name: str
    language: int
    exec_mode: int
    exec_unit: Optional[PyExecutionUnit]
    input: Optional[PyDataObject]
    stream_input: Optional[PyStreamDataObject]
    streaming_result: bool

    def __init__(
        self,
        task_id: str,
        name: str,
        language: int,
        exec_mode: int,
        exec_unit: Optional[PyExecutionUnit] = None,
        input: Optional[PyDataObject] = None,
        stream_input: Optional[PyStreamDataObject] = None,
        streaming_result: bool = False,
    ) -> None: ...
    def __str__(self) -> str: ...

@final
@final
class PyLocalEnvironmentConfig:
    """Configuration for local environment execution.

    Attributes:
        custom_id: Optional custom identifier for the environment
        working_dir: Optional working directory path
        envs: Optional environment variables mapping
    """

    custom_id: Optional[str]
    working_dir: Optional[str]
    envs: Optional[Dict[str, str]]

    def __init__(
        self,
        custom_id: Optional[str] = None,
        working_dir: Optional[str] = None,
        envs: Optional[Dict[str, str]] = None,
    ) -> None: ...

@final
class PyDockerEnvironmentConfig:
    """Configuration for Docker container execution.

    Attributes:
        image: Docker image name/tag
        custom_id: Optional custom identifier for the environment
        working_dir: Optional working directory inside container
        mounts: List of volume mount tuples (source, target)
        envs: Optional environment variables mapping
    """

    image: str
    custom_id: Optional[str]
    working_dir: Optional[str]
    mounts: List[tuple[str, str]]
    envs: Optional[Dict[str, str]]

    def __init__(
        self,
        image: str,
        custom_id: Optional[str] = None,
        working_dir: Optional[str] = None,
        mounts: Optional[List[tuple[str, str]]] = None,
        envs: Optional[Dict[str, str]] = None,
    ) -> None: ...

@final
class PyEnvironmentConfig:
    """Configuration for task execution environment.

    This class represents the complete environment configuration for task execution,
    supporting both local and Docker environments.

    Attributes:
        local: Optional local environment configuration
        docker: Optional Docker environment configuration
        envs: Optional global environment variables mapping
    """

    local: Optional[PyLocalEnvironmentConfig]
    docker: Optional[PyDockerEnvironmentConfig]
    envs: Optional[Dict[str, str]]

    def __init__(
        self,
        local: Optional[PyLocalEnvironmentConfig] = None,
        docker: Optional[PyDockerEnvironmentConfig] = None,
        envs: Optional[Dict[str, str]] = None,
    ) -> None: ...
    def env_id(self) -> str: ...
    def clone_new(self, custom_id: Optional[str] = None) -> Self: ...

@final
class PyTaskStateInfo:
    """Information about the state of a task.

    Contains details about task execution status, output, and other relevant information.

    Attributes:
        task_id: Unique identifier for the task
        state: Current state of the task
        start_time: Task start timestamp
        end_time: Task end timestamp
        worker_id: ID of the worker executing the task
        output: Optional task output data
        exit_code: Task exit code
        stdout: Standard output from the task
        stderr: Standard error from the task
    """

    task_id: str
    state: int
    start_time: int
    end_time: int
    worker_id: str
    output: Optional[PyDataObject]
    exit_code: int
    stdout: str
    stderr: str

@final
class PyTaskCpuConfig:
    """CPU resource configuration for tasks.

    Attributes:
        cpu_cores: Number of CPU cores
        cpu_shares: CPU shares for container
        cpu_quota: CPU quota for container
        cpu_period: CPU period for container scheduling
    """

    def __init__(
        self,
        cpu_cores: Optional[float] = None,
        cpu_shares: Optional[int] = None,
        cpu_quota: Optional[int] = None,
        cpu_period: Optional[int] = None,
    ) -> None: ...

@final
class PyTaskCallArgs:
    """Arguments for task execution.

    This class encapsulates the arguments passed to a task during execution.

    Attributes:
        data: Optional data object containing the task input data and parameters
    """

    data: Optional[PyDataObject]

    def __init__(self, data: Optional[PyDataObject] = None) -> None: ...

@final
class PyTaskMemoryConfig:
    """Memory resource configuration for tasks.

    Attributes:
        memory_limit: Maximum memory usage in bytes
    """

    def __init__(self, memory_limit: Optional[int] = None) -> None: ...

@final
class PyTaskNetworkConfig:
    """Network resource configuration for tasks.

    Attributes:
        enable_network: Whether to enable network access
        ingress_limit_kbps: Incoming bandwidth limit in kbps
        egress_limit_kbps: Outgoing bandwidth limit in kbps
        allowed_hosts: List of allowed host addresses
        allowed_ports: List of allowed port ranges
    """

    def __init__(
        self,
        enable_network: Optional[bool] = None,
        ingress_limit_kbps: Optional[int] = None,
        egress_limit_kbps: Optional[int] = None,
        allowed_hosts: Optional[List[str]] = None,
        allowed_ports: Optional[List[Tuple[int, int]]] = None,
    ) -> None: ...

@final
class PyTaskFilePerms:
    """File permissions configuration.

    The permissions are represented as a bitfield where:
    - 0b01 represents READ permission
    - 0b10 represents WRITE permission
    """

    def __init__(self, inner: Optional[int] = None) -> None: ...

@final
class PyTaskFsConfig:
    """Filesystem configuration for tasks.

    Attributes:
        preopens: List of pre-opened directories with their permissions
            Each tuple contains (host_path, container_path, dir_perms, file_perms)
        fs_size_limit: Maximum filesystem size in bytes
        temp_dir: Path to temporary directory
    """

    def __init__(
        self,
        preopens: Optional[List[Tuple[str, str, int, int]]] = None,
        fs_size_limit: Optional[int] = None,
        temp_dir: Optional[str] = None,
    ) -> None: ...

@final
class PyTaskInstanceLimits:
    """Instance limits configuration for tasks.

    Attributes:
        max_instances: Maximum number of instances
        max_tables: Maximum number of tables
        max_table_elements: Maximum number of table elements
    """

    def __init__(
        self,
        max_instances: Optional[int] = None,
        max_tables: Optional[int] = None,
        max_table_elements: Optional[int] = None,
    ) -> None: ...

@final
class PyTaskResourceConfig:
    """Complete resource configuration for tasks.

    This class combines all resource configurations including CPU, memory,
    network, filesystem, and instance limits into a single configuration object.

    It also includes task timeout, environment variables, and other task settings.

    Attributes:
        cpu: CPU resource configuration
        memory: Memory resource configuration
        network: Network resource configuration
        fs: Filesystem configuration
        instance_limits: Instance limits configuration
        timeout_ms: Task timeout in milliseconds
        env_vars: Environment variables for the task
    """

    def __init__(
        self,
        cpu: Optional[PyTaskCpuConfig] = None,
        memory: Optional[PyTaskMemoryConfig] = None,
        network: Optional[PyTaskNetworkConfig] = None,
        fs: Optional[PyTaskFsConfig] = None,
        instance_limits: Optional[PyTaskInstanceLimits] = None,
        timeout_ms: Optional[int] = None,
        env_vars: Optional[List[Tuple[str, str]]] = None,
    ) -> None: ...

@final
class PyTaskHandle:
    """Handle for controlling and interacting with a running task.

    This class provides methods to execute, control and interact with tasks in the system.
    It supports different execution modes including direct running, code execution, and
    function calls with custom inputs.

    The handle maintains a connection to the underlying task and provides async methods
    for task manipulation.
    """

    def task_id(self) -> str:
        """Get the task ID for the handle.

        Returns:
            Task ID for the handle
        """
        ...

    async def run(
        self, args: PyTaskCallArgs, resources: Optional[PyTaskResourceConfig] = None
    ) -> PyDataObject:
        """Execute the task with given arguments.

        Args:
            args: Task arguments encapsulated in PyTaskCallArgs
            resources: Optional resource configuration for the task

        Returns:
            PyDataObject containing the task execution results

        Raises:
            PyTypeError: If required data is missing
            PyRuntimeError: If task execution fails
        """
        ...

    async def exec(
        self,
        lang: str,
        code: str,
        decode: bool = True,
        resources: Optional[PyTaskResourceConfig] = None,
    ) -> PyDataObject:
        """Execute code in specified language.

        Args:
            lang: Programming language identifier
            code: Code to execute
            decode: Whether to decode the execution result
            resources: Optional resource configuration

        Returns:
            PyDataObject containing the execution results

        Raises:
            PyRuntimeError: If code execution fails
        """
        ...

    async def exec1(
        self,
        lang: str,
        code: str,
        call_name: str,
        input: bytes,
        encode: bool,
        decode: bool = True,
        resources: Optional[PyTaskResourceConfig] = None,
    ) -> List[PyDataObject]:
        """Execute code with a specific function call and input.

        Args:
            lang: Programming language identifier
            code: Code to execute
            call_name: Name of the function to call
            input: Input data for the function
            encode: Whether to encode the input
            decode: Whether to decode the output
            resources: Optional resource configuration

        Returns:
            List of PyDataObject containing the execution data and output

        Raises:
            PyRuntimeError: If execution fails
        """
        ...

    async def stop(self) -> None:
        """Stop the task execution.

        Raises:
            PyRuntimeError: If stopping the task fails
        """
        ...

@final
@final
class PyTaskStateInfo:
    """Information about the state of a task.

    Contains details about task execution status, output, and other relevant information.

    Attributes:
        task_id: Unique identifier for the task
        state: Current state of the task
        start_time: Task start timestamp
        end_time: Task end timestamp
        worker_id: ID of the worker executing the task
        output: Optional task output data
        exit_code: Task exit code
        stdout: Standard output from the task
        stderr: Standard error from the task
    """

    task_id: str
    state: int
    start_time: int
    end_time: int
    worker_id: str
    output: Optional[PyDataObject]
    exit_code: int
    stdout: str
    stderr: str

@final
class PyLyric:
    """Main class for managing lyric tasks and workers.

    This class provides the core functionality for task submission, worker management,
    and distributed execution coordination.
    """

    def __init__(self, config: PyConfig) -> None: ...
    def start_worker(self, config: PyWorkerConfig) -> None: ...
    """Start a worker node with the given configuration.
    
    Args:
        config: Worker configuration
    """

    def start_driver(self, config: PyDriverConfig) -> None: ...
    """Start the driver node with the given configuration.
    
    Args:
        config: Driver configuration
    """

    def stop(self) -> None: ...
    """Stop the lyric driver node.
    
    If the node is a driver, this will stop the driver and all workers.
    """

    def set_callback(self, callback: Callable[..., Any]) -> None: ...
    """Set a callback function for task worker.
    
    Args:
        callback: Callback function to set
    """

    async def submit_task(
        self,
        py_task_info: PyTaskInfo,
        environment_config: Optional[PyEnvironmentConfig] = None,
    ) -> PyTaskHandle: ...
    """Submit a task for execution.
    
    Args:
        py_task_info: Task information
        environment_config: Optional environment configuration
        
    Returns:
        PyTaskHandle for the submitted task
    """

    async def submit_task_async(
        self,
        py_task_info: PyTaskInfo,
        callback: Callable[[Union[PyTaskStateInfo, int]], Any],
        environment_config: Optional[PyEnvironmentConfig] = None,
    ) -> str: ...
    """Submit a task for execution asynchronously.
    
    Now working yet.
    
    Args:
        py_task_info: Task information
        callback: Callback function for task state updates
        environment_config: Optional environment configuration
        
    Returns:
        Task ID for the submitted task
    """

    def join(self) -> None: ...
    """Wait for the lyric driver to finish.
    
    This method will block the current thread until the driver stops.
    """
