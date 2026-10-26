# no-exception-swallowing-tests (GR008)

## What it does

Flags test exception handlers that only pass, return without a value, or skip.
These handlers prevent the caught exception from failing the test. `...` and
a handler whose only statement is a string literal count too.

## Why

An unexpected exception should fail a test. Catching it and passing or skipping
can hide a regression. A smoke test needs no explicit assertion if an unexpected
exception can still reach the test runner.

## Example

Let an unexpected connection failure reach the runner:

```diff
 def test_fetch_returns_payload():
-    try:
-        payload = fetch("/status")
-    except ConnectionError:
-        pytest.skip("service unavailable")
+    payload = fetch("/status")
     assert payload["status"] == "ok"
```

If the exception is expected, assert it instead:

```diff
+import pytest
+
+
 def test_rejects_invalid_port():
-    try:
+    with pytest.raises(ValueError):
         parse_port("invalid")
-    except ValueError:
-        pass
```

### Rule scope

- Test definitions have names starting with `test` in `test_*.py` or `*_test.py`
  files. They must not be nested inside another function. Every `try` inside a
  test counts, including inside loops, `with` blocks, or nested functions.
- Each `except` or `except*` handler is checked separately. An optional leading
  docstring does not exempt it. Recognized skip calls are `pytest.skip`,
  `self.skipTest`, and `cls.skipTest`.
- Any other handler statement, including an assertion, re-raise, logging,
  `pytest.fail`, or a return with a value, puts the handler outside the rule.
- A `try` with an `else` exempts handlers without skip calls. This permits the
  hand-written `assertRaises` pattern; Gruff does not inspect the `else` body.
  A skip stays flagged because it never reaches that `else`.

## When to suppress

Decide predictable skips before the call with `pytest.mark.skipif` or
`pytest.importorskip`. For a known flaky test, use a rerun plugin. Cleanup may use
`Path.unlink(missing_ok=True)` or `shutil.rmtree(..., ignore_errors=True)` when
ignoring those errors is intentional.

Adding a log line or replacing the handler with `contextlib.suppress` hides the
finding without restoring the test's ability to fail. Ruff's unsafe `SIM105` fix
can suggest that rewrite.

Suppress a runtime skip when the condition cannot be checked before the call.
Put the reason on the `except` line:

```python
def test_probe_reports_capability():
    try:
        probe = open_camera()
    except CameraUnavailable:  # noqa: GR008 -- the camera is optional on CI runners
        pytest.skip("no camera attached")
    assert probe.is_ready
```

Gruff only inspects the handler. It can also flag a test whose `try` ends with an
unconditional `raise`, `self.fail`, or `pytest.fail` that the handler cannot catch.
Suppress that finding with a reason if the test already fails as intended.
