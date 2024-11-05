# Lyric

A Rust-powered secure sandbox for multi-language code execution, leveraging WebAssembly to provide high-performance runtime isolation for AI applications.

## ✨ Features

- 🛡️ **Secure Isolation**: Sandboxed environment based on WebAssembly for reliable runtime isolation
- 🚀 **High Performance**: Built with Rust to ensure optimal execution performance
- 🌐 **Multi-language Support**: Run Python, JavaScript, and more in a unified environment
- 🔌 **Easy Integration**: Clean Python bindings for seamless integration with existing projects
- 🎯 **AI-Optimized**: Runtime environment specifically optimized for AI applications

## 🔧 Requirements

- Python >= 3.8

## 🚀 Quick Start

### Installation

**Install Lyric via pip:**

```bash
pip install "lyric-py>=0.1.4-rc0"
```

**Install default Python webassembly worker:**

```bash
pip install "lyric-py-worker>=0.1.4-rc0"
```

**Install default JavaScript webassembly worker:**

```bash
pip install "lyric-js-worker>=0.1.4-rc0"
```

### Basic Usage

```python
import asyncio
from lyric import DefaultLyricDriver

python_code = """
def add(a, b):
    return a + b
result = add(1, 2)
print(result)
"""

js_code = """
console.log('Hello from JavaScript!');
"""

async def main():
    lcd = DefaultLyricDriver(host="localhost", log_level="ERROR")
    lcd.start()

    # Load workers(default: Python, JavaScript)
    await lcd.lyric.load_default_workers()

    # Execute Python code
    py_res = await lcd.exec(python_code, "python")
    print(py_res)

    # Execute JavaScript code
    js_res = await lcd.exec(js_code, "javascript")
    print(js_res)

    # Stop the driver
    lcd.stop()

asyncio.run(main())
```

### Function Execution

```python
import asyncio
import json
from lyric import DefaultLyricDriver

py_func = """
def message_handler(message_dict):
    user_message = message_dict.get("user_message")
    ai_message = message_dict.get("ai_message")
    return {
        "user": user_message,
        "ai": ai_message,
        "all": [user_message, ai_message],
        "custom": "custom",
        "handler_language": "python",
    }
"""

js_func = """
function message_handler(message_dict) {
    return {
        user: message_dict.user_message,
        ai: message_dict.ai_message,
        all: [message_dict.user_message, message_dict.ai_message],
        custom: "custom",
        handler_language: "javascript",
    };
}
"""
async def main():
    lcd = DefaultLyricDriver(host="localhost", log_level="ERROR")
    lcd.start()

    # Load workers(default: Python, JavaScript)
    await lcd.lyric.load_default_workers()

    input_data = {
        "user_message": "Hello from user",
        "ai_message": "Hello from AI",
    }
    input_bytes = json.dumps(input_data).encode("utf-8")
    
    py_res = await lcd.exec1(py_func, input_bytes, "message_handler", lang="python")
    # Get the result of the function execution
    result_dict = py_res.output
    print("Python result:", result_dict)
    print(f"Full output: {py_res}")

    js_res = await lcd.exec1(js_func, input_bytes, "message_handler", lang="javascript")
    # Get the result of the function execution
    result_dict = js_res.output
    print("JavaScript result:", result_dict)
    print(f"Full output: {js_res}")

    # Stop the driver
    lcd.stop()

asyncio.run(main())
```

## Advanced Usage

### Execution With Specific Resources

```python
import asyncio
from lyric import DefaultLyricDriver, PyTaskResourceConfig, PyTaskFsConfig, PyTaskMemoryConfig

lcd = DefaultLyricDriver(host="localhost", log_level="ERROR")
lcd.start()

python_code = """
import os

# List the files in the root directory
root = os.listdir('/tmp/')
print("Files in the root directory:", root)

# Create a new file in the home directory
with open('/home/new_file.txt', 'w') as f:
    f.write('Hello, World!')
"""

async def main():
    # Load workers(default: Python, JavaScript)
    await lcd.lyric.load_default_workers()
    
    dir_read, dir_write = 1, 2
    file_read, file_write = 3, 4
    resources = PyTaskResourceConfig(
        fs=PyTaskFsConfig(
            preopens=[
                # Mount current directory in host to "/tmp" in the sandbox with read permission
                (".", "/tmp", dir_read, file_read),
                # Mount "/tmp" in host to "/home" in the sandbox with read and write permission
                ("/tmp", "/home", dir_read | dir_write, file_read | file_write),
            ]
        ),
        memory=PyTaskMemoryConfig(
            # Set the memory limit to 30MB
            memory_limit=30 * 1024 * 1024  # 30MB in bytes
        )
    )

    py_res = await lcd.exec(python_code, "python", resources=resources)
    assert py_res.exit_code == 0, "Python code should exit with 0"

    # Stop the driver
    lcd.stop()

asyncio.run(main())
```

## Examples

- [Notebook-Qick Start](examples/notebook/lyric_quick_start.ipynb): A Jupyter notebook demonstrating how to use Lyric to execute Python and JavaScript code.
- [Notebook-Sandbox Execution](examples/notebook/lyric_sandbox_verification.ipynb): A Jupyter notebook demonstrating how to use Lyric to execute Python and JavaScript code in a sandboxed environment.

## 🤝 Contributing

We welcome Issues and Pull Requests! Please check out our [Contributing Guidelines](.github/CONTRIBUTING.md) for more information.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details

## ⭐️ Show Your Support

If you find Lyric helpful, please give us a star! It helps others discover this project.