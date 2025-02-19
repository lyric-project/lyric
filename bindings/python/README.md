# python

Python bindings for Lyric.

## Build From Source

### Prerequisites

- Python >= 3.10
- Rust >= 1.83.0
- wasm-tools >= 1.220.0
- rollup >= 4.25.0
- protobuf >= 3.17.3

1. Install The `wasm-tools` toolchain.

```bash
cargo install wasm-tools
```
2. Install the `rollup` toolchain.

```bash
npm install --global rollup
```

3. Add  the `wasm32-wasip1` target.

```bash
rustup target add wasm32-wasi
```

4. Install `protobuf` compiler.

For Ubuntu:
```bash
sudo apt install protobuf-compiler
```

### Build Core Library and Bindings

```bash
pushd lyric-py
rye run maturin build --release
popd
```

### Build Worker And Components

```bash
make build
```

Then you can find the generated wheel file in the `./dist` directory.
