import argparse
import os
import sys
import time

import asyncio
from time import sleep
from typing import List

from lyric import (
    PyLyric,
    PyTaskInfo,
    PyExecutionUnit,
    PyConfig,
    PyDriverConfig,
    PyEnvironmentConfig,
    PyLocalEnvironmentConfig,
    PyDockerEnvironmentConfig,
)
from lyric import from_python_iterator, DEFAULT_WORKER_PATH
from lyric.task import (
    TaskInfo,
    MyDummyTask,
    NormalCodeTask,
    ExecutableTask,
    unwrapper_task_output,
)


current_dir = os.path.dirname(os.path.abspath(__file__))
# py_worker = os.path.join(current_dir, "python_worker.py")
py_worker = DEFAULT_WORKER_PATH
code_file = os.path.join(current_dir, "code_to_run.py")

with open(code_file, "r") as f:
    my_code = f.read()


def print_current_time():
    while True:
        print(f"Current time: {time.time()}")
        time.sleep(1)


async def submit_task(
    pp, num, task_info: TaskInfo, environment_config=None, max_concurrency=100
):
    task_id = task_info.task_id

    async def worker(queue, worker_id: int):
        while True:
            i = await queue.get()
            new_task_id = f"{task_id}-{worker_id}-{i}"
            try:
                has_stop = False

                def call_back(task_state_info):
                    nonlocal has_stop
                    try:
                        is_iter = hasattr(task_state_info, "__next__")
                        print(f"Callback: {task_state_info}, is_iter: {is_iter}")
                        if is_iter:
                            for item in iter(task_state_info):
                                out = unwrapper_task_output(item)
                                print(
                                    f"Item: {out.data}, stderr: {out.stderr}, stdout: {out.stdout}"
                                )
                        else:
                            out = unwrapper_task_output(task_state_info)
                            print(
                                f"Task output:: {out.data}, stderr: {out.stderr}, stdout: {out.stdout}"
                            )
                        results[i] = task_state_info
                    except Exception as e:
                        print(f"Error in callback: {e}")
                    finally:
                        has_stop = True

                task_info.task_id = new_task_id
                await pp.submit_task_async(
                    task_info.to_core(),
                    call_back,
                    environment_config=environment_config,
                )
                print(f"[Python-{i}] Submitted task")
                while not has_stop:
                    await asyncio.sleep(0.01)
                print(f"[Python-{i}] Task completed")
                # results[i] = await pp.submit_task_async_old(task_info.to_core(), environment_config=environment_config)
                # print(f"[Python-{i}] Submitted task with result: ", result)
            finally:
                print(f"Task {new_task_id} finally done")
                queue.task_done()

    start_time = time.time()
    queue = asyncio.Queue()
    results = [None] * num

    # Create worker tasks
    workers = [
        asyncio.create_task(worker(queue, w_id)) for w_id in range(max_concurrency)
    ]

    # Add tasks to the queue
    for i in range(num):
        await queue.put(i)

    # Wait for all tasks to complete
    await queue.join()

    # Cancel worker tasks
    for w in workers:
        w.cancel()

    # Wait for all worker tasks to be cancelled
    await asyncio.gather(*workers, return_exceptions=True)

    print(f"Total time: {time.time() - start_time}")
    return results


async def old_submit_task(pp, num, task_info):
    async def process_task(i):
        result = await pp.submit_task_async(task_info)
        # print(f"[Python-{i}] Submitted task with result: ", result)
        return result

    start_time = time.time()
    tasks = [process_task(i) for i in range(num)]
    results = await asyncio.gather(*tasks)
    print(f"Total time: {time.time() - start_time}")
    return results


class PythonTask(ExecutableTask):
    def __init__(self, data):
        self.data = data

    def run(self):
        return self.data


class IterTask(ExecutableTask):
    def __init__(self, data: List[int]):
        self.data = data

    @property
    def streaming_result(self):
        return True

    def run(self):
        # return from_python_iterator(iter(self.data))
        return iter(self.data)


class SyncIterTask(ExecutableTask):
    @property
    def streaming_result(self):
        return True

    def run(self):
        def gen():
            for i in range(10):
                print(i)
                yield str(i)
                sleep(2)

        # return from_python_iterator(gen())
        return gen()


class AsyncIterTask(ExecutableTask):
    @property
    def streaming_result(self):
        return True

    def run(self):
        async def gen():
            print("Start async generator")
            for i in range(10):
                print(i)
                yield str(i)
                sleep(2)

        print("Create async generator")
        # loop = get_or_create_event_loop()
        # return from_python_iterator(iter(AsyncToSyncIterator(gen(), loop)))
        return gen()


if __name__ == "__main__":

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--address", type=str, default="[::1]:15670", help="Address to bind to"
    )
    parser.add_argument(
        "--driver_address", type=str, default=None, help="Driver address to connect to"
    )
    parser.add_argument("--log_file", type=str, default=None)
    parser.add_argument("--network_mode", type=str, default="host")

    args = parser.parse_args()
    address = args.address
    driver_address = args.driver_address

    # task = MyDummyTask()
    # task = PythonTask({"data": "Hello from PythonTask", "task_type": "dict"})
    task = IterTask(["Hello", "from", "PythonTask", "task_type", "list"])
    # task = AsyncIterTask()
    # task = SyncIterTask()
    # task = NormalCodeTask(my_code)
    task_info = TaskInfo.from_task("my_python_task", "unique_task_id", 0, task)

    py_config = PyConfig(
        host="0.0.0.0",
        port=15670,
        is_driver=True,
        worker_port_start=15671,
        worker_port_end=16670,
        maximum_workers=4,
        minimum_workers=1,
        worker_start_commands={"PYTHON": f"{sys.executable} {py_worker}"},
        # worker_start_commands={"PYTHON": f"python3 /app/python_worker.py"},
        # For alpine container
        # worker_start_commands={"PYTHON": f"python /app/python_worker.py"},
        eventloop_worker_threads=100,
    )
    config = PyDriverConfig()
    pp = PyLyric(py_config)
    pp.start_driver(config)

    environment_config = PyEnvironmentConfig(
        local=PyLocalEnvironmentConfig(envs={"LYRIC_CORE_LOG_ANSICOLOR": "false"}),
        # docker=PyDockerEnvironmentConfig(image="py-lyric-base:latest", mounts=[(current_dir, "/app")]),
        # docker=PyDockerEnvironmentConfig(image="py-lyric-base-alpine:latest", mounts=[(current_dir, "/app")])
    )

    # res = asyncio.run(submit_task(pp, 5000, task_info))
    # New event loop
    loop = asyncio.new_event_loop()
    # res = loop.run_until_complete(submit_task(pp, 22, task_info))
    task_num = 1
    res = loop.run_until_complete(
        submit_task(pp, task_num, task_info, environment_config=environment_config)
    )
    pp.stop()
    print("Stopped driver")
    if task_num < 100:
        print("Final results: ", res)
    # result = pp.submit_task(task_info)
    # print("[Python] Submitted task with result: ", result)
    pp.join()
    # time.sleep(10)
