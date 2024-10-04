import argparse
import uuid
from lyric import PyLyric, PyTaskInfo, PyConfig, PyWorkerConfig

from lyric.task import TaskInfo


def on_message(msg: PyTaskInfo):
    print(f"MSG: {msg}")
    task_info = TaskInfo.from_core(msg)
    print(f"Received message: {task_info}")
    return "Executed successfully"


if __name__ == "__main__":

    parser = argparse.ArgumentParser()
    parser.add_argument("--host", type=str, default=None, help="Host")
    parser.add_argument("--port", type=int, default=None, help="Port")
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
    # print(args)
    node_id = str(uuid.uuid4())
    driver_address = args.driver_address

    py_config = PyConfig(
        # host=args.host,
        port=args.port,
        is_driver=False,
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
