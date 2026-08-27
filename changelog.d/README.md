# Changelog fragments

Each user-facing change ships as one file in this directory instead of editing
`CHANGELOG.md` directly, so concurrent pull requests never conflict.

- Filename: `<PR number>.<type>.md`, where `<type>` is one of `added`,
  `changed`, `deprecated`, `removed`, `fixed`, or `security`. A second fragment
  for the same PR and type gets a counter suffix such as `1234.fixed.2.md`.
- Content: one line without a bullet or PR link, such as `Fixed the thing`.
  Towncrier adds the link from the filename.

To release version `X.Y.Z` on `YYYY-MM-DD`:

1. Run `uvx --from towncrier==25.8.0 towncrier build --yes --version X.Y.Z --date YYYY-MM-DD`.
2. Add `[X.Y.Z]: https://github.com/wkentaro/gruff/compare/v<previous>...vX.Y.Z`
   to the link list at the bottom of `CHANGELOG.md`, then update `[Unreleased]`
   to compare `vX.Y.Z...main`.
3. Commit the updated changelog and deleted fragments, then tag that commit.
