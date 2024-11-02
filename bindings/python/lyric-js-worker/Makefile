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

.PHONY: build-wasm
build-wasm: setup
	# Activate virtual environment and run setup.py
	# to src directory and run setup.py
	cd ../../javascript/lyric-js-worker && npm run build
	cp ../../javascript/lyric-js-worker/javascript_worker.wasm src/lyric_js_worker/

.PHONY: build
build: build-wasm
	rye build