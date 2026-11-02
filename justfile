set shell := ["bash", "-eu", "-o", "pipefail", "-c"]
set script-interpreter := ["bash", "-eu", "-o", "pipefail"]
set positional-arguments
set default-list
set minimum-version := "1.58.0"

ruff := "uvx --from ruff==0.16.4 ruff"
mkdocs := "uvx --from mkdocs==1.6.1 --with mkdocs-material==9.7.7 mkdocs"
towncrier := "uvx --from towncrier==25.8.0 towncrier"
mdformat := "uvx --from mdformat==1.0.0 mdformat"

# Lint code
lint:
    {{ ruff }} format --check
    {{ ruff }} check
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features --locked -- -D warnings
    cargo run --locked -- check .

# Format code
format:
    {{ ruff }} format
    {{ ruff }} check --fix
    cargo fmt --all

# Run tests
test *cargo_args:
    cargo test --all-targets --locked "$@"

# Build the documentation site
docs:
    {{ mkdocs }} build --strict

# Prepare a release
[script]
release version=env("VERSION", ""):
    version="$1"
    if test -z "$version"; then
        fragments=$(find changelog.d -maxdepth 1 -type f \( \
            -name "*.added.md" -o -name "*.changed.md" -o \
            -name "*.deprecated.md" -o -name "*.removed.md" -o \
            -name "*.fixed.md" -o -name "*.security.md" \))
        latest=$(git tag --sort=-v:refname |
            grep -Em1 "^v[0-9]+\.[0-9]+\.[0-9]+$" || true)
        if test -n "$fragments" && test -n "$latest"; then
            latest_version=${latest#v}
            major=${latest_version%%.*}
            remainder=${latest_version#*.}
            minor=${remainder%%.*}
            patch=${remainder#*.}
            if test "$major" = 0; then
                if grep -q '\*\*Breaking:\*\*' $fragments; then
                    next=0.$((minor + 1)).0
                else
                    next=0.$minor.$((patch + 1))
                fi
            elif grep -q '\*\*Breaking:\*\*' $fragments; then
                next=$((major + 1)).0.0
            elif find changelog.d -maxdepth 1 -type f \( \
                -name "*.added.md" -o -name "*.changed.md" -o \
                -name "*.deprecated.md" -o -name "*.removed.md" \) \
                -print -quit | grep -q .; then
                next=$major.$((minor + 1)).0
            else
                next=$major.$minor.$((patch + 1))
            fi
            echo "suggested: just release $next" >&2
        else
            echo "usage: just release X.Y.Z" >&2
        fi
        echo "recent releases:" >&2
        git tag --sort=-v:refname | sed -n "1,5{s/^/  /;p}" >&2
        exit 1
    fi
    perl -pi -e "s/^version = \".*\"/version = \"$version\"/" Cargo.toml pyproject.toml
    cargo check --quiet
    {{ towncrier }} build --yes --version "$version"
    {{ mdformat }} CHANGELOG.md
    git add CHANGELOG.md
    printf "\n\033[1;32mNext steps\033[0m\n"
    echo "  git commit -am \"chore: prep $version release\""
    echo "  git tag v$version"
    echo "  git push origin main v$version"
