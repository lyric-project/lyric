import sys
import json
import asyncio

# import cloudpickle as pickle
# import dill as pickle
# import lyric_task
# from lyric_task import pickle as pickle
import cloudpickle
import click
import rich
import shortuuid
import lyric_task

from lyric_task.log import IOCapture
from lyric_task.std import *

from lyric_py_task import exports, imports
from lyric_py_task.imports import msgpack


count = 0

"""
Import packages:
https://github.com/bytecodealliance/componentize-py/issues/91


Reduce the size of the generated wasm file:
wasm-tools strip -a
wasm-tools print python_task.wasm | wasm-tools strip -a -o python_task_stripped.wasm
"""


class InterpreterTask(exports.InterpreterTask):
    def run(
        self, script: exports.types.InterpreterRequest
    ) -> exports.types.InterpreterResponse:
        """
        Raises: `interpreter_task.types.Err(interpreter_task.imports.str)`
        """
        global count
        print(f"[Python-InterpreterTask] script: {script}")
        print("[Python-InterpreterTask] count:", count)
        count += 1
        with IOCapture() as capture:
            exec(script.code)
            stdout, stderr = capture.get_output()

        result_dict = {
            "lang": "Python",
            "protocol": 1,
            "content": "Execute script successfully",
            "stdout": stdout,
            "stderr": stderr,
            "success": True,
        }
        serialized = msgpack.serialize(
            json.dumps(result_dict, ensure_ascii=False).encode("utf-8")
        )
        test_result = msgpack.deserialize(serialized)
        print(f"[Python-InterpreterTask] test_result: {test_result}")
        return exports.types.InterpreterResponse(protocol=1, data=serialized)

    def run1(
        self, request: exports.types.InterpreterRequest, call_name: str, input: bytes
    ) -> exports.types.InterpreterOutputResponse:
        print(f"[Python-InterpreterTask] script: {request}")
        print(f"[Python-InterpreterTask] call_name: {call_name}")

        # 执行用户脚本
        exec_globals = {}
        exec(request.code, exec_globals)

        # 检查call_name是否在执行后的全局变量中
        if call_name not in exec_globals:
            raise ValueError(f"函数 {call_name} 未在脚本中定义")

        # 获取call_name对应的函数
        target_function = exec_globals[call_name]

        # 解码输入
        input_json = msgpack.deserialize(input).decode("utf-8")
        input_dict = json.loads(input_json)

        # 执行函数并捕获输出
        with IOCapture() as capture:
            try:
                output = target_function(input_dict)
                stdout, stderr = capture.get_output()
                success = True
            except Exception as e:
                err_msg = str(e)
                stdout, stderr = capture.get_output()
                stderr = stderr or ""
                stderr += f"\nException:\n{err_msg}"
                success = False

        result_dict = {
            "lang": "Python",
            "protocol": 1,
            "content": "Execute script successfully",
            "success": success,
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
        return exports.types.InterpreterOutputResponse(
            protocol=1, data=result_bytes, output=output_bytes
        )


class BinaryTask(exports.BinaryTask):
    def run(self, request: exports.types.BinaryRequest) -> exports.types.BinaryResponse:
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
            return exports.types.BinaryResponse(protocol=1, data=dumps(task_res))
        else:
            raise Exception(f"Unsupported protocol {request.protocol}")


def loads(data):
    from lyric_task.pickle import loads

    # return pickle.loads(data)
    return loads(data)


def dumps(data):
    from my_py_lyric.pickle import dumps

    # return pickle.dumps(data)
    return dumps(data)
