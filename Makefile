UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
    SED := $(shell command -v gsed 2>/dev/null)
    ifeq ($(SED),)
        $(error GNU sed (gsed) not found on macOS. \
			Install with: brew install gnu-sed)
    endif
else
    SED := sed
endif

.PHONY: help
help: ## Ask for help!
	@grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; \
		{printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'

.PHONY: build
build: ## Build the project in debug mode
	cargo build

.PHONY: build-release
build-release: ## Build the project in release mode
	cargo build --release

.PHONY: check
check: ## Check code for compilation errors
	cargo check

.PHONY: check-format
check-format: ## Check code formatting
	cargo +nightly fmt --check

.PHONY: check-format-toml
check-format-toml: ## Check TOML formatting
	taplo fmt --check

.PHONY: format
format: ## Format code
	cargo +nightly fmt

.PHONY: format-toml
format-toml: ## Format TOML files
	taplo fmt

.PHONY: lint
lint: ## Run linter
	cargo clippy -- -D warnings

.PHONY: test
test: ## Run tests
	cargo test

.PHONY: test-verbose
test-verbose: ## Run tests with output
	cargo test -- --nocapture

.PHONY: doc
doc: ## Open documentation in browser
	cargo doc --no-deps --open

.PHONY: doc-build
doc-build: ## Build documentation
	cargo doc --no-deps

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean

.PHONY: setup
setup: ## Setup development environment
	rustup component add clippy
	rustup toolchain install nightly
	rustup component add --toolchain nightly rustfmt
	cargo install taplo-cli

.PHONY: publish-dry
publish-dry: ## Dry-run publish to crates.io
	cargo publish --dry-run

.PHONY: publish
publish: ## Publish to crates.io
	cargo publish

.PHONY: fix-trailing-whitespace
fix-trailing-whitespace: ## Remove trailing whitespaces
	@echo "Removing trailing whitespaces from all files..."
	@find . -type f \( \
		-name "*.rs" -o -name "*.toml" -o -name "*.md" \
		-o -name "*.yaml" -o -name "*.yml" \
		-o -name "*.sh" -o -name "*.json" \) \
		-not -path "./target/*" \
		-not -path "./.git/*" \
		-exec sh -c \
			'echo "Processing: $$1"; \
			$(SED) -i -e "s/[[:space:]]*$$//" "$$1"' \
			_ {} \; && \
		echo "Trailing whitespaces removed."

.PHONY: check-trailing-whitespace
check-trailing-whitespace: ## Check for trailing whitespaces
	@echo "Checking for trailing whitespaces..."
	@files_with_trailing_ws=$$(find . -type f \( \
		-name "*.rs" -o -name "*.toml" -o -name "*.md" \
		-o -name "*.yaml" -o -name "*.yml" \
		-o -name "*.sh" -o -name "*.json" \) \
		-not -path "./target/*" \
		-not -path "./.git/*" \
		-exec grep -l '[[:space:]]$$' {} + 2>/dev/null \
		|| true); \
	if [ -n "$$files_with_trailing_ws" ]; then \
		echo "Files with trailing whitespaces found:"; \
		echo "$$files_with_trailing_ws" | sed 's/^/  /'; \
		echo ""; \
		echo "Run 'make fix-trailing-whitespace' to fix."; \
		exit 1; \
	else \
		echo "No trailing whitespaces found."; \
	fi

.PHONY: lint-shell
lint-shell: ## Lint shell scripts using shellcheck
	shellcheck .github/scripts/*.sh
