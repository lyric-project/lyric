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

$(VENV)/.venv-timestamp: app-requirements.txt
	# Create new virtual environment if setup.py has changed
	python3 -m venv $(VENV)
	$(VENV_BIN)/pip install --upgrade pip
	$(VENV_BIN)/pip install -r app-requirements.txt
	touch $(VENV)/.venv-timestamp


.PYHONY: build-task
build-task: setup
	# to src directory and run setup.py
	cd ../lyric-task && rye build
	$(VENV_BIN)/pip install --force-reinstall ../dist/lyric_task-*.whl

.PHONY: build-wasm
build-wasm: setup build-task
	# Activate virtual environment and run setup.py
	# to src directory and run setup.py
	cd src && source ../$(VENV_BIN)/activate && componentize-py \
		-d ./wit -w lyric-py-task componentize worker \
		-o python_worker_fat.wasm \
		&& wasm-tools print python_worker_fat.wasm | \
        wasm-tools strip -a -o lyric_py_worker/python_worker.wasm \
        && rm python_worker_fat.wasm

.PYTHON: build-bindings
build-bindings: setup
	cd src && source ../$(VENV_BIN)/activate && componentize-py \
		-d ./wit -w lyric-py-task bindings .

.PHONY: build
build: build-wasm
	rye build