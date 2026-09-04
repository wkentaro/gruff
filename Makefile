RUFF := uvx --from ruff==0.16.4 ruff
MKDOCS := uvx --from mkdocs==1.6.1 --with mkdocs-material==9.7.7 mkdocs
TOWNCRIER := uvx --from towncrier==25.8.0 towncrier
MDFORMAT := uvx --from mdformat==1.0.0 mdformat

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
		fragments=$$(find changelog.d -maxdepth 1 -type f \( \
			-name "*.added.md" -o -name "*.changed.md" -o \
			-name "*.deprecated.md" -o -name "*.removed.md" -o \
			-name "*.fixed.md" -o -name "*.security.md" \)); \
		latest=$$(git tag --sort=-v:refname | \
			grep -E "^v[0-9]+\.[0-9]+\.[0-9]+$$" | head -1); \
		if test -n "$$fragments" && test -n "$$latest"; then \
			version=$${latest#v}; \
			major=$${version%%.*}; \
			remainder=$${version#*.}; \
			minor=$${remainder%%.*}; \
			patch=$${remainder#*.}; \
			if grep -q '\*\*Breaking:\*\*' $$fragments; then \
				next=$$((major + 1)).0.0; \
			elif find changelog.d -maxdepth 1 -type f \( \
				-name "*.added.md" -o -name "*.changed.md" -o \
				-name "*.deprecated.md" -o -name "*.removed.md" \) | grep -q .; then \
				next=$$major.$$((minor + 1)).0; \
			else \
				next=$$major.$$minor.$$((patch + 1)); \
			fi; \
			echo "suggested: make release VERSION=$$next" >&2; \
		else \
			echo "usage: make release VERSION=X.Y.Z" >&2; \
		fi; \
		echo "recent releases:" >&2; \
		git tag --sort=-v:refname | head -5 | sed "s/^/  /" >&2; \
		exit 1; \
	}
	$(call exec,perl -pi -e 's/^version = ".*"/version = "$(VERSION)"/' Cargo.toml pyproject.toml)
	$(call exec,cargo check --quiet)
	$(call exec,$(TOWNCRIER) build --yes --version $(VERSION))
	$(call exec,$(MDFORMAT) CHANGELOG.md && git add CHANGELOG.md)
	@printf "\n\033[1;32mNext steps\033[0m\n"
	@echo "  git commit -am \"chore: prep $(VERSION) release\""
	@echo "  git tag v$(VERSION)"
	@echo "  git push origin main v$(VERSION)"
