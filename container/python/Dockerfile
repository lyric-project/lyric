# Build Python binding for Lyric
FROM lyric-rust-base:ubuntu22.04-python3.10-rust as builder

WORKDIR /app

COPY . .

WORKDIR /app/bindings/lyric-py
RUN maturin build --release --interpreter python3.10

# For the final image
FROM ubuntu:22.04

# Install Python and other dependencies
RUN apt-get update && apt-get install -y \
    python3.10 \
    python3-pip \
    telnet \
    net-tools \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy wheel files from the builder stage
COPY --from=builder /app/target/wheels/*.whl .
COPY --from=builder /app/bindings/lyric-py/examples/python_driver.py .
COPY --from=builder /app/bindings/lyric-py/examples/python_worker.py .

# Install the wheel files
RUN pip3 install *.whl