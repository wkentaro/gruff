# no-exception-swallowing-tests (GR008)

## What it does

Flags an `except` clause inside a test definition whose body statements are each one of `pass`, an `...` expression, a bare `return`, a `pytest.skip(...)` call, or a `self.skipTest(...)` or `cls.skipTest(...)` call; an `except*` clause counts the same as an `except`, a docstring as the first statement does not disqualify the handler, and a handler whose whole body is a single string literal is flagged too, since nothing in it can fail. A test definition is a function or method whose name starts with `test` in a file named `test_*.py` or `*_test.py` and not lexically nested inside another function's body, and every `try` lexically inside it counts, at any depth, including inside loops, `with` blocks, and functions nested in the test. Each swallowing handler is its own finding, so a sibling handler that re-raises does not exempt it.

Any other statement in the handler puts it outside the rule: a re-raise, an assertion about the exception, logging, `pytest.fail`, or a `return` with a value. So do handlers in non-test functions, test-named functions in files that are not test files, and `try` statements that have only a `finally` clause. A handler that makes no skip call — one that only passes, `...`s, bare-returns, or holds only a string literal — is outside the rule when its `try` carries an `else` clause: that is the hand-rolled `assertRaises` idiom, whose `else` block runs only when nothing was raised and so holds the failure. The exemption does not depend on what the `else` body contains, and a handler that skips stays flagged even with an `else`, since the skip path never reaches it.

## Why

A test that catches an exception and then passes, returns, or skips cannot fail on the exception it was written to catch: the failure arrives, the handler absorbs it, and the suite stays green or skips. The test still counts as coverage while verifying nothing.

A test with no assertion at all is not the problem. An unexpected exception fails a bare-call smoke test, which is the implicit assertion that makes it worth running. The swallowing handler is what removes that assertion, and it is what this rule flags.

## Example

```diff
 def test_fetch_returns_payload():
-    try:
-        payload = fetch("/status")
-    except ConnectionError:
-        pytest.skip("service unavailable")
+    payload = fetch("/status")
     assert payload["status"] == "ok"
```

## When to suppress

Let the exception reach the runner. When one is genuinely expected, say so: assert it with `pytest.raises`, decide the skip before the test runs with `pytest.mark.skipif` or `pytest.importorskip`, or leave a known-flaky test to a rerun plugin. A handler that only cleans up usually has a spelling that removes it outright: `Path.unlink(missing_ok=True)`, `shutil.rmtree(..., ignore_errors=True)`. Rewriting a flagged handler as `contextlib.suppress`, or adding a log line to it, silences the rule without giving the test back its ability to fail — and Ruff `SIM105` recommends exactly that suppress rewrite as an unsafe-gated fix, so taking it hides the finding rather than answering it.

The rule reads only the handler, so a `try` body ending in an unconditional `raise`, `self.fail(...)`, or `pytest.fail(...)` past a handler too narrow to catch it is flagged even though the test does fail; suppress those with `# noqa: GR008`.

An `except` clause that skips is worth keeping when the condition it detects cannot be evaluated before the call it guards. Suppress it with `# noqa: GR008 -- reason` on the `except` line:

```python
def test_probe_reports_capability():
    try:
        probe = open_camera()
    except CameraUnavailable:  # noqa: GR008 -- the camera is optional on CI runners
        pytest.skip("no camera attached")
    assert probe.is_ready
```
