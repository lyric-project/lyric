"""Python Worker Implementation Using WASI.

Import packages:
https://github.com/bytecodealliance/componentize-py/issues/91

Reduce the size of the generated wasm file:
wasm-tools strip -a
wasm-tools print python_task.wasm | wasm-tools strip -a -o python_task_stripped.wasm
"""

import asyncio
import json
import sys
import traceback
from typing import List

import click
import cloudpickle
import lyric_task
import rich
import shortuuid
from lyric_py_task import Err, Ok, Optional, exports
from lyric_py_task.imports import (
    lyric_task_types,
    msgpack,
    outgoing_handler,
    wasi_http_types,
)
from lyric_py_task.imports.streams import (
    InputStream,
    OutputStream,
    StreamError_Closed,
    StreamError_LastOperationFailed,
)
from lyric_py_task.imports.wasi_http_types import FutureTrailers, IncomingBody
from lyric_task.log import IOCapture
from lyric_task.std import *


class InterpreterTask(exports.InterpreterTask):
    def run(
        self, script: lyric_task_types.InterpreterRequest
    ) -> lyric_task_types.InterpreterResponse:
        """
        Raises: `interpreter_task.types.Err(interpreter_task.imports.str)`
        """
        print(f"[Python-InterpreterTask] script: {script}")
        success = False
        with IOCapture() as capture:
            try:
                exec(script.code)
                success = True
            except Exception as e:
                print(f"[Python-InterpreterTask] Exception: {e}")
                traceback.print_exc()
                success = False
            stdout, stderr = capture.get_output()

        result_dict = {
            "lang": "Python",
            "protocol": 1,
            "content": "Execute script successfully",
            "success": success,
            "exit_code": 0 if success else 1,
            "stdout": stdout,
            "stderr": stderr,
        }
        serialized = msgpack.serialize(
            json.dumps(result_dict, ensure_ascii=False).encode("utf-8")
        )
        test_result = msgpack.deserialize(serialized)
        print(f"[Python-InterpreterTask] test_result: {test_result}")
        return lyric_task_types.InterpreterResponse(protocol=1, data=serialized)

    def run1(
        self, request: lyric_task_types.InterpreterRequest, call_name: str, input: bytes
    ) -> lyric_task_types.InterpreterOutputResponse:
        print(f"[Python-InterpreterTask] script: {request}")
        print(f"[Python-InterpreterTask] call_name: {call_name}")

        # Execute the user script
        exec_globals = {}
        exec(request.code, exec_globals)

        # Check if the function is defined in the script
        if call_name not in exec_globals:
            raise ValueError(f"Function {call_name} is not defined in the script")

        # Get the target function
        target_function = exec_globals[call_name]

        # Deserialize the input
        input_json = msgpack.deserialize(input).decode("utf-8")
        input_dict = json.loads(input_json)

        # Execute the function and capture the output
        with IOCapture() as capture:
            try:
                output = target_function(input_dict)
                stdout, stderr = capture.get_output()
                success = True
            except Exception as e:
                err_msg = str(e)
                traceback.print_exc()
                stdout, stderr = capture.get_output()
                stderr = stderr or ""
                stderr += f"\nException:\n{err_msg}"
                success = False

        result_dict = {
            "lang": "Python",
            "protocol": 1,
            "content": "Execute script successfully",
            "success": success,
            "exit_code": 0 if success else 1,
            "stdout": stdout,
            "stderr": stderr,
        }
        result_bytes = msgpack.serialize(
            json.dumps(result_dict, ensure_ascii=False).encode("utf-8")
        )
        # output of the function must be a dict
        output_dict = output
        output_bytes = msgpack.serialize(
            json.dumps(output_dict, ensure_ascii=False).encode("utf-8")
        )
        return lyric_task_types.InterpreterOutputResponse(
            protocol=1, data=result_bytes, output=output_bytes
        )


class BinaryTask(exports.BinaryTask):
    def run(
        self, request: lyric_task_types.BinaryRequest
    ) -> lyric_task_types.BinaryResponse:
        """
        Raises: `binary_task.types.Err(binary_task.imports.str)`
        """
        if request.protocol == 1:
            print(f"Function: {request.data}")
            print(f"The request data type: {type(request.data)}")
            callable_task = loads(request.data)
            print(f"Type of callable_task: {type(callable_task)}")
            print(callable_task)
            task_res = callable_task()
            print(f"[Python-BinaryTask] task_res: {task_res}")
            return lyric_task_types.BinaryResponse(protocol=1, data=dumps(task_res))
        else:
            raise Exception(f"Unsupported protocol {request.protocol}")


def loads(data):
    from lyric_task.pickle import loads

    # return pickle.loads(data)
    return loads(data)


def dumps(data):
    from lyric_task.pickle import dumps

    # return pickle.dumps(data)
    return dumps(data)
