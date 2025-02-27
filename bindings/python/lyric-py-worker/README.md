# lyric-py-worker

A Python worker implementation for the lyric.

It allows you to run your python code in a wasm environment.

## Installation

```bash
pip install lyric-py-worker
```

## How To Add Your Dependencies

1. You can add your dependencies to the `app-requrements.txt` file.

```bash
echo "chardet" >> app-requirements.txt
```

2. Import the dependencies `src/worker.py`

Modify the `src/worker.py` file to import your dependencies.

```python
import asyncio
# ...
# Import your dependencies at the top of the file
import chardet
# ...
import shortuuid
from lyric_task.std import *

```

3. Build the worker

```bash
make build
```

After building the worker, you can see the wheel file in the `../dist` directory.

```bash
ls ../dist/lyric_py_worker-*.whl
../dist/lyric_py_worker-0.1.7rc0-py3-none-any.whl
```
The output may be like the above.

4. Install the worker

You can install your worker using the following command.

```bash
pip install --force-reinstall lyric_py_worker-0.1.7rc0-py3-none-any.whl
```
Make sure to replace the wheel file name with the one you have and the path to the wheel file.