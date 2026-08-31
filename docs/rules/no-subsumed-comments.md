# no-subsumed-comments (GR007)

## What it does

Flags a one-line own-line comment below the first five physical lines when every content word appears in the window it annotates: the next line carrying code, with comments counting as blank, plus the three physical lines after it. Matching ignores stopwords, splits identifiers on camel-case and uppercase parts, applies a small fixed synonym map for check, loop and iteration, and return wording, and reads a window line containing `(` as the token `call`.

Multi-line comment blocks, trailing comments, directives, section dividers, annotation forms — a leading colon, a leading `name:`, or a parenthesized comma list — and comments with fewer than two content words are outside the rule, as are the contents of any string token that begins on an assertion or comparison line.

## Why

A comment that repeats its statement costs a reader a second pass over the same fact, and it silently goes stale: renaming the code leaves the restatement behind, and nothing catches it. Deleting the comment loses nothing, because the statement below already carries every word of it.

The comments worth keeping are the ones the code cannot state: why a value was chosen, which constraint forces a workaround, what a caller must not assume. Restricting comments to that content makes each surviving one worth stopping for.

## Example

```diff
 from browser.session import browser_session


 async def click_element(*, index: int) -> None:
     """Click the element at the given index."""
-    # Get the element
     element = await browser_session.get_dom_element_by_index(index)
     await element.click()
```

## When to suppress

Delete a comment that restates the statement beneath it, and replace it with the reasoning the statement cannot carry when there is any.

The rule misjudges three shapes: a comment whose words all appear in the code below but which carries a reason the code cannot; a scenario label above a data literal whose strings repeat it, such as `# Choice type with default.` over a `pytest.mark.parametrize` table; and a wrapped assertion whose expected string sits below the comparison line, where the string is no longer masked. A trailing directive is stripped before matching, so a `# noqa` for another rule does not shield a subsumed comment. Suppress with `# noqa: GR007 -- reason`:

```python
from app.generated import register_generated_client

settings = load_settings()
telemetry = start_telemetry(settings)

# Register the generated client  # noqa: GR007 -- required by the code generator
generated_client = register_generated_client()
```
