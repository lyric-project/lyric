# Lyric

A Rust-powered secure sandbox for multi-language code execution, leveraging WebAssembly to provide high-performance runtime isolation for AI applications.

## ✨ Features

- 🛡️ **Secure Isolation**: Sandboxed environment based on WebAssembly for reliable runtime isolation
- 🚀 **High Performance**: Built with Rust to ensure optimal execution performance
- 🌐 **Multi-language Support**: Run Python, JavaScript, and more in a unified environment
- 🔌 **Easy Integration**: Clean Python bindings for seamless integration with existing projects
- 🎯 **AI-Optimized**: Runtime environment specifically optimized for AI applications

## 🚀 Quick Start

### Installation

**Install Lyric via pip:**

```bash
pip install lyric-py
```

**Install default Python webassembly worker:**

```bash
pip install lyric-py-worker
```

**Install default JavaScript webassembly worker:**

```bash
pip install lyric-js-worker
```

### Basic Usage

```python
import asyncio
from lyric import DefaultLyricDriver

lcd = DefaultLyricDriver(host="localhost", log_level="ERROR")
lcd.start()

# Load workers(default: Python, JavaScript)
asyncio.run(lcd.lyric.load_default_workers())

# Execute Python code
python_code = """
def add(a, b):
    return a + b
result = add(1, 2)
print(result)
"""

py_res = asyncio.run(lcd.exec(python_code, "python"))
print(py_res)

# Execute JavaScript code
js_code = """
console.log('Hello from JavaScript!');
"""

js_res = asyncio.run(lcd.exec(js_code, "javascript"))
print(js_res)

# Stop the driver
lcd.stop()
```

## 🔧 Requirements

- Python >= 3.8

## 🤝 Contributing

We welcome Issues and Pull Requests! Please check out our [Contributing Guidelines](.github/CONTRIBUTING.md) for more information.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details

## ⭐️ Show Your Support

If you find Lyric helpful, please give us a star! It helps others discover this project.