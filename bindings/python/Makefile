.DEFAULT_GOAL := help

# Define subproject paths
PY_WORKER_DIR = lyric-py-worker
JS_WORKER_DIR = lyric-js-worker
TASK_WORKER_DIR = lyric-task
COMPONENT_TS_TRANSPILED_DIR = lyric-component-ts-transpiling

.PHONY: all
all: build ## Build all subprojects

.PHONY: build
build: build-task build-py-worker build-js-worker component-ts-tra ## Build all subprojects

.PHONY: build-task
build-task: ## Build the task dependency
	@echo "Building task..."
	$(MAKE) -C $(TASK_WORKER_DIR) build

.PHONY: build-py-worker
build-py-worker: ## Build the Python worker
	@echo "Building Python worker..."
	$(MAKE) -C $(PY_WORKER_DIR) build


.PHONY: build-js-worker
build-js-worker: ## Build the JavaScript worker
	@echo "Building JavaScript worker..."
	$(MAKE) -C $(JS_WORKER_DIR) build

.PHONY: component-ts-tra
component-ts-tra: ## Build the component-ts-transpiling
	@echo "Building component-ts-transpiling..."
	$(MAKE) -C $(COMPONENT_TS_TRANSPILED_DIR) build

.PHONY: clean
clean: ## Clean all projects
	@echo "Cleaning all projects..."
	rm -rf dist


.PHONY: publish-pypi
publish-pypi: build ## Publish all packages to PyPI
	@echo "Publishing to PyPI..."
	@if [ -z "$(TWINE_USERNAME)" ] || [ -z "$(TWINE_PASSWORD)" ]; then \
		echo "Error: TWINE_USERNAME and TWINE_PASSWORD environment variables must be set"; \
		exit 1; \
	fi
	@echo "Uploading all distributions from dist/"
	twine upload dist/*

.PHONY: publish-testpypi
publish-testpypi: build ## Publish all packages to TestPyPI
	@echo "Publishing to TestPyPI..."
	@if [ -z "$(TWINE_USERNAME)" ] || [ -z "$(TWINE_PASSWORD)" ]; then \
		echo "Error: TWINE_USERNAME and TWINE_PASSWORD environment variables must be set"; \
		exit 1; \
	fi
	@echo "Uploading all distributions from dist/"
	rye run twine upload --repository testpypi dist/*

.PHONY: help
help:  ## Display this help screen
	@echo "Available commands:"
	@grep -E '^[a-z.A-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}' | sort