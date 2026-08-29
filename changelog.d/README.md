# Changelog fragments

Each user-facing change ships as one file here instead of editing
`CHANGELOG.md` directly, so concurrent pull requests never conflict.

Name the file `<PR number>.<type>.md`, where `<type>` is one of `added`,
`changed`, `deprecated`, `removed`, `fixed`, or `security`. A second fragment
for the same PR and type takes a counter suffix: `123.fixed.2.md`.

Write the entry as a single line without a bullet or PR link; towncrier adds
the link from the filename. Prefix `**Breaking:**` for changes that bump the
major version.

Release a minor version for any ready backward-compatible improvement and a
patch for backward-compatible fixes; there is no minimum release size.

To release version `X.Y.Z`:

1. Set the version to `X.Y.Z` in `Cargo.toml` and `pyproject.toml`, then run
   `cargo check` so `Cargo.lock` matches; the release build is `--locked`.
1. Run `uvx --from towncrier==25.8.0 towncrier build --yes --version X.Y.Z`.
1. Commit the updated changelog and deleted fragments, then tag that commit.

Pushing the tag publishes to PyPI and creates the GitHub release from the
matching `CHANGELOG.md` section.
