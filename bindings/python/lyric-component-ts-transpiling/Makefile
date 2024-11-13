.DEFAULT_GOAL := help

SHELL=/bin/bash
VENV = .venv

# Detect the operating system and set the virtualenv bin directory
ifeq ($(OS),Windows_NT)
	VENV_BIN=$(VENV)/Scripts
else
	VENV_BIN=$(VENV)/bin
endif

setup: $(VENV)/bin/activate

$(VENV)/bin/activate: $(VENV)/.venv-timestamp

$(VENV)/.venv-timestamp:
	# Create new virtual environment if setup.py has changed
	python3 -m venv $(VENV)
	$(VENV_BIN)/pip install --upgrade pip
	touch $(VENV)/.venv-timestamp

.PYHONY: build-task
build-task: setup
	# to src directory and run setup.py
	cd ../lyric-task && rye build
	$(VENV_BIN)/pip install --force-reinstall ../dist/lyric_task-*.whl

.PHONY: build-wasm
build-wasm: build-task
	# Activate virtual environment and run setup.py
	# to src directory and run setup.py
	cd ../../../components/rust/component-ts-transpiling && cargo build --release --target wasm32-wasip1
	cp ../../../target/wasm32-wasip1/release/component_ts_transpiling.wasm src/lyric_component_ts_transpiling

.PHONY: build
build: build-wasm
	rye build