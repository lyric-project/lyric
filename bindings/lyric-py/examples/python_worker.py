import argparse

import cloudpickle
import logging

from lyric import PyLyric, PyTaskInfo, PyConfig, PyWorkerConfig

from lyric.task import TaskInfo

logger = logging.getLogger("lyric")


def on_message(msg: PyTaskInfo):
    print(f"MSG on worker: {msg}")
    task_info = TaskInfo.from_core(msg)
    task = task_info.to_task()
    exec_locals = {}
    exec_globals = {
        # "__name__": "__main__",
        # "__file__": "<string>",
        # "__builtins__": __builtins__,
    }
    # if isinstance(task_info.detail, NormalCode):
    #     exec(task_info.detail.code, exec_globals, exec_locals)
    # elif isinstance(task_info.detail, SerializingData):
    #     task = cloudpickle.loads(task_info.detail.data)
    #     task(task_info)
    # else:
    #     raise ValueError(f"Unknown detail type: {task_info.detail}")
    # logger.error('Something bad happened')
    logger.info(f"Received message: {task_info}")
    # logger.error(f"Error Received message: {task_info}")
    return task()


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
        default="http://[::1]:15670",
        help="Driver address to connect to",
    )
    parser.add_argument("--log_file", type=str, default=None)
    parser.add_argument("--network_mode", type=str, default="host")

    args, _ = parser.parse_known_args()
    node_id = args.node_id
    driver_address = args.driver_address
    public_host = args.public_host

    py_config = PyConfig(
        # host=args.host,
        port=args.port,
        is_driver=False,
        public_host=public_host,
        worker_port_start=15671,
        worker_port_end=16670,
        maximum_workers=10,
        minimum_workers=1,
        node_id=node_id,
    )
    config = PyWorkerConfig(driver_address=driver_address)
    pp = PyLyric(py_config)

    pp.start_worker(config)
    pp.set_callback(on_message)
    pp.join()
    # time.sleep(100000000)
