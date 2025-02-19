.DEFAULT_GOAL := help
SHELL=/bin/bash
VENV = .venv

# Detect the operating system and set the virtualenv bin directory
ifeq ($(OS),Windows_NT)
	VENV_BIN=$(VENV)/Scripts
else
	VENV_BIN=$(VENV)/bin
endif

# Function to check if a command exists
CHECK_CMD = command -v $(1) >/dev/null 2>&1

setup: $(VENV)/bin/activate

$(VENV)/bin/activate: $(VENV)/.venv-timestamp

$(VENV)/.venv-timestamp:
	# Create new virtual environment if setup.py has changed
	python3 -m venv $(VENV)
	$(VENV_BIN)/pip install --upgrade pip
	touch $(VENV)/.venv-timestamp

.PHONY: check-node
check-node:
	@if ! $(call CHECK_CMD,node); then \
		echo "Node.js not found. Checking version managers..."; \
		if $(call CHECK_CMD,fnm); then \
			echo "fnm found, checking installed versions..."; \
			if fnm list | grep -q "v[0-9]"; then \
				echo "Node.js versions found in fnm, using latest..."; \
				LATEST_VERSION=$$(fnm list | grep "v[0-9]" | tail -n1 | awk '{print $$2}'); \
				fnm use $$LATEST_VERSION; \
			else \
				echo "No Node.js versions found in fnm, installing latest..."; \
				fnm install --latest && fnm use latest; \
			fi; \
		elif $(call CHECK_CMD,nvm); then \
			echo "nvm found, checking installed versions..."; \
			if [ -s "$$HOME/.nvm/nvm.sh" ]; then \
				. "$$HOME/.nvm/nvm.sh"; \
				if nvm ls | grep -q "v[0-9]"; then \
					echo "Node.js versions found in nvm, using latest..."; \
					nvm use node; \
				else \
					echo "No Node.js versions found in nvm, installing latest..."; \
					nvm install node && nvm use node; \
				fi; \
			fi; \
		else \
			echo "Error: Neither Node.js nor a version manager (fnm/nvm) was found."; \
			echo "Please install Node.js, fnm, or nvm to continue."; \
			exit 1; \
		fi \
	else \
		echo "Node.js is already installed."; \
	fi

.PYHONY: build-task
build-task: setup
	# to src directory and run setup.py
	cd ../lyric-task && rye build
	$(VENV_BIN)/pip install --force-reinstall ../dist/lyric_task-*.whl

.PHONY: build-wasm
build-wasm: build-task check-node
	# Activate virtual environment and run setup.py
	# to src directory and run setup.py
	# Print current directory
	echo "Current directory: $(PWD)"
	cd ../../javascript/lyric-js-worker && npm install && npm run build
	cp ../../javascript/lyric-js-worker/javascript_worker.wasm src/lyric_js_worker/

.PHONY: build
build: build-wasm
	rye build