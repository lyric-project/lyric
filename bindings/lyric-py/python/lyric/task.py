import uuid
from abc import ABC, abstractmethod

import cloudpickle
from typing import Optional, Any
from enum import Enum

from ._py_lyric import (
    PyTaskInfo,
    PyExecutionUnit,
    PyDataObject,
    PyTaskOutputObject,
    PyTaskStateInfo,
)


class Language(Enum):
    PYTHON = 0
    NODEJS = 1
    SHELL = 2


class DataFormat(Enum):
    RAW = 0
    PICKLE = 1
    MSGPACK = 2
    PROTOBUF = 3

    @classmethod
    def from_int(cls, value: int):
        return cls(value)


class TaskOutputObject:
    def __init__(self, output: Any, stdout: str, stderr: str):
        self.data = output
        self.stdout = stdout
        self.stderr = stderr


def wrapper_task_output(
    output: Any, stdout: str | None, stderr: str | None
) -> PyTaskOutputObject:
    object_id = str(uuid.uuid4())
    code_data = cloudpickle.dumps(output)
    code_object = PyDataObject(
        object_id=object_id, format=DataFormat.PICKLE.value, data=code_data
    )
    return PyTaskOutputObject(code_object, stdout=stdout or "", stderr=stderr or "")


def unwrapper_task_output(output: PyTaskStateInfo) -> TaskOutputObject:
    data = output.output
    code_format = DataFormat(data.format)
    code_data = data.data
    if code_format == DataFormat.PICKLE:
        return TaskOutputObject(
            cloudpickle.loads(bytes(code_data)), output.stdout, output.stderr
        )
    else:
        raise ValueError(f"Unsupported data format: {code_format}")


def unwrap_task_output_data(output: PyDataObject) -> Any:
    code_format = DataFormat(output.format)
    code_data = output.data
    if code_format == DataFormat.PICKLE:
        return cloudpickle.loads(bytes(code_data))
    else:
        raise ValueError(f"Unsupported data format: {code_format}")


class BaseTask(ABC):
    @abstractmethod
    def run(self):
        raise NotImplementedError

    @property
    def streaming_result(self):
        return False

    def __call__(self):
        return self.run()

    @classmethod
    def to_execution_unit(cls, task: "BaseTask") -> PyExecutionUnit:
        unit_id = str(uuid.uuid4())
        if isinstance(task, NormalCodeTask):
            code_data_format = DataFormat.RAW
            code_data = task.code.encode("utf-8")
        elif isinstance(task, ExecutableTask):
            code_data_format = DataFormat.PICKLE
            code_data = cloudpickle.dumps(task)
        else:
            raise ValueError(f"Unsupported task type: {type(task)}")
        code_object = PyDataObject(
            object_id=str(uuid.uuid4()), format=code_data_format.value, data=code_data
        )
        return PyExecutionUnit(
            unit_id=unit_id, language=Language.PYTHON.value, code=code_object
        )


class TaskInfo:
    def __init__(
        self,
        name: str,
        task_id: str,
        language: int,
        exec_mode: int,
        exec_unit: Optional[PyExecutionUnit] = None,
        input: Optional[PyDataObject] = None,
        streaming_result: bool = False,
    ):
        self.name = name
        self.task_id = task_id
        self.language = language
        self.exec_mode = exec_mode
        self.exec_unit = exec_unit
        self.input = input
        self.streaming_result = streaming_result

    @classmethod
    def from_task(cls, name: str, task_id: str, exec_mode: int, task: BaseTask):
        exec_unit = BaseTask.to_execution_unit(task)
        return cls(
            name=name,
            task_id=task_id,
            language=Language.PYTHON.value,
            exec_mode=exec_mode,
            exec_unit=exec_unit,
            streaming_result=task.streaming_result,
        )

    @classmethod
    def from_core(cls, core_task_info: PyTaskInfo):
        return cls(
            task_id=core_task_info.task_id,
            name=core_task_info.name,
            language=core_task_info.language,
            exec_mode=core_task_info.exec_mode,
            exec_unit=core_task_info.exec_unit,
            input=core_task_info.input,
        )

    def to_core(self) -> PyTaskInfo:
        return PyTaskInfo(
            task_id=self.task_id,
            name=self.name,
            language=self.language,
            exec_mode=self.exec_mode,
            exec_unit=self.exec_unit,
            input=self.input,
            streaming_result=self.streaming_result,
        )

    def to_task(self) -> BaseTask:
        if not self.exec_unit:
            raise ValueError("No execution unit provided")
        if self.language != Language.PYTHON.value:
            raise ValueError("Only Python tasks are supported on Python worker")
        code_object = self.exec_unit.code
        if not code_object:
            raise ValueError("No code object provided")
        code_format = DataFormat(code_object.format)
        code_data = code_object.data
        if not code_data:
            raise ValueError("No code data provided")
        code_data = bytes(code_data)
        if code_format == DataFormat.RAW:
            str_code = code_data.decode("utf-8")
            return NormalCodeTask(str_code)
        elif code_format == DataFormat.PICKLE:
            # TODO: Support passing input data
            return cloudpickle.loads(code_data)
        else:
            raise ValueError(f"Unsupported data format: {self.exec_unit.data_format}")

    def __str__(self):
        return f"TaskInfo(name={self.name}, task_id={self.task_id}, language={self.language}, exec_mode={self.exec_mode})"


class NormalCodeTask(BaseTask):
    def __init__(self, code: str):
        self.code = code

    def run(self):
        exec(self.code)


class ExecutableTask(BaseTask, ABC):
    pass


class MyDummyTask(ExecutableTask):
    def run(self):
        print("Hello from MyDummyTask")
        return "Executed successfully, return a simple string"
