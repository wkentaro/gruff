RUFF := uvx --from ruff==0.16.4 ruff
MKDOCS := uvx --from mkdocs==1.6.1 --with mkdocs-material==9.7.7 mkdocs
TOWNCRIER := uvx --from towncrier==25.8.0 towncrier

.PHONY: help format lint test docs release
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

docs:  # Build the documentation site
	$(call exec,$(MKDOCS) build --strict)

release:  # Prepare a release: make release VERSION=X.Y.Z
	@test -n "$(VERSION)" || { \
		echo "usage: make release VERSION=X.Y.Z" >&2; \
		echo "recent releases:" >&2; \
		git tag --sort=-v:refname | head -5 | sed "s/^/  /" >&2; \
		exit 1; \
	}
	$(call exec,perl -pi -e 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml pyproject.toml)
	$(call exec,cargo check --quiet)
	$(call exec,$(TOWNCRIER) build --yes --version $(VERSION))
	@printf "\n\033[1;32mNext steps\033[0m\n"
	@echo "  git commit -am \"chore: prep $(VERSION) release\""
	@echo "  git tag v$(VERSION)"
	@echo "  git push origin main v$(VERSION)"
