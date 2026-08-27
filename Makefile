RUFF := uvx --from ruff==0.16.4 ruff

.PHONY: help format lint test
.DEFAULT_GOAL := help

define exec
	@printf '\033[1;36m%s\033[0m\n' '$(1)'
	@$(1)
endef

help:
	@printf '\033[1;32mAvailable targets\033[0m\n'
	@awk 'BEGIN {FS = ":.*# "} /^[a-zA-Z_-]+:.*# / {printf "  \033[1;36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

lint:  # Lint code
	$(call exec,$(RUFF) format --check)
	$(call exec,$(RUFF) check)
	$(call exec,cargo fmt --all -- --check)
	$(call exec,cargo clippy --all-targets --all-features --locked -- -D warnings)
	$(call exec,cargo run --locked -- check .)

format:  # Format code
	$(call exec,$(RUFF) format)
	$(call exec,$(RUFF) check --fix)
	$(call exec,cargo fmt --all)

test:  # Test code
	$(call exec,cargo test --all-targets --locked)
	$(call exec,python3 -m tests.tools.release_notes_test)
