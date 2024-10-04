# Build Python binding for Lyric
FROM lyric-rust-base:alpine-python3.10-rust AS builder

WORKDIR /app

# Install patchelf and other necessary build tools
RUN apk add --no-cache patchelf gcc musl-dev

COPY . .

WORKDIR /app/bindings/lyric-py
RUN pip install maturin[patchelf]
RUN maturin build --release --interpreter python3.10

# For the final image
FROM python:3.10-alpine

# Install additional dependencies
RUN apk add --no-cache \
    busybox-extras \
    net-tools

WORKDIR /app

# Copy wheel files from the builder stage
COPY --from=builder /app/target/wheels/*.whl .
COPY --from=builder /app/bindings/lyric-py/examples/python_driver.py .
COPY --from=builder /app/bindings/lyric-py/examples/python_worker.py .

# Install the wheel files
RUN pip3 install *.whl

# Set the entrypoint or CMD as needed
# CMD ["python3", "your_main_script.py"]