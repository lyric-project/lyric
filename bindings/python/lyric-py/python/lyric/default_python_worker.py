import argparse
import logging

from lyric import PyConfig, PyLyric, PyTaskInfo, PyWorkerConfig, from_python_iterator
from lyric.task import TaskInfo, wrapper_task_output
from lyric_task.log import IOCapture, capture_iterator_output
from lyric_task.utils import (
    AsyncToSyncIterator,
    get_or_create_event_loop,
    is_async_iterator,
    is_iterator,
)

logger = logging.getLogger("lyric")


def on_message(msg: PyTaskInfo):
    task_info = TaskInfo.from_core(msg)
    task = task_info.to_task()
    logger.info(f"Received message on python worker: {task_info}")

    with IOCapture() as capture:
        result = task()
        stdout, stderr = capture.get_output()
    if is_iterator(result):
        return from_python_iterator(
            capture_iterator_output(result, wrapper_task_output)
        )
    elif is_async_iterator(result):
        loop = get_or_create_event_loop()
        new_iter = iter(AsyncToSyncIterator(result, loop))
        return from_python_iterator(
            capture_iterator_output(new_iter, wrapper_task_output)
        )
    else:
        return wrapper_task_output(result, stdout, stderr)


if __name__ == "__main__":

    parser = argparse.ArgumentParser()
    parser.add_argument("--host", type=str, default=None, help="Host")
    parser.add_argument("--port", type=int, default=None, help="Port")
    parser.add_argument(
        "--public-host",
        type=str,
        default=None,
        help="Public host to register to driver",
    )
    parser.add_argument(
        "--node-id", type=str, default=None, help="Node ID of the worker"
    )
    parser.add_argument(
        "--driver-address",
        type=str,
        default=None,
        help="Driver address to connect to",
    )
    parser.add_argument("--log_file", type=str, default=None)
    parser.add_argument("--network_mode", type=str, default="host")

    args, _ = parser.parse_known_args()
    node_id = args.node_id
    public_host = args.public_host

    py_config = PyConfig(
        host=args.host,
        port=args.port,
        is_driver=False,
        public_host=public_host,
        worker_port_start=15671,
        worker_port_end=16670,
        maximum_workers=10,
        minimum_workers=1,
        node_id=node_id,
    )
    config = PyWorkerConfig(driver_address=args.driver_address)
    pp = PyLyric(py_config)

    pp.start_worker(config)
    pp.set_callback(on_message)
    pp.join()
