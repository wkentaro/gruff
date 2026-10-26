# no-subsumed-comments (GR007)

## What it does

Flags a standalone, single-line comment when every content word appears in the
code it describes. Checks comments below the first five physical lines against
the next line carrying code and the three physical lines after it.

## Why

A comment that repeats the code adds another description to read and maintain.
Keep comments that explain a reason, constraint, or assumption the code cannot
express.

## Example

Delete the comment that repeats the lookup below it:

```diff
 from browser.session import browser_session


 async def click_element(*, index: int) -> None:
     """Click the element at the given index."""
-    # Get the element
     element = await browser_session.get_dom_element_by_index(index)
     await element.click()
```

### Rule scope

Comments count as blank lines when locating the first code line. Matching ignores
stopwords, splits camel-case and uppercase identifier parts, applies a fixed
synonym map for check, loop, iteration, and return wording, and treats a window
line containing `(` as the token `call`.

The rule excludes comment blocks, trailing comments, directives, section dividers,
comments with fewer than two content words, and annotation forms that start with
`:`, start with `name:`, or contain a parenthesized comma list. It masks string
tokens that begin on an assertion or comparison line.

## When to suppress

Delete restatements. If the comment explains a reason the code cannot express,
keep it and suppress the finding:

```python
from app.generated import register_generated_client

settings = load_settings()
telemetry = start_telemetry(settings)

# Register the generated client  # noqa: GR007 -- required by the code generator
generated_client = register_generated_client()
```

False positives can include scenario labels above data literals, or wrapped
assertions whose expected string begins below the comparison line. A trailing
`noqa` for another rule does not exempt the comment; use `noqa: GR007` with a reason.
