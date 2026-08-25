import json
import os
import sqlite3
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any
from typing import Dict
from typing import Sequence
from typing import Tuple


def _insert_codex_item(
    connection: sqlite3.Connection,
    thread_id: str,
    turn_id: str,
    ordinal: int,
    timestamp: int,
    item: Dict[str, Any],
) -> None:
    connection.execute(
        "INSERT INTO thread_items VALUES (?, ?, ?, ?, ?, ?)",
        (thread_id, turn_id, ordinal, timestamp, item["type"], json.dumps(item)),
    )


def _add_codex_sandwich(
    state: sqlite3.Connection,
    history: sqlite3.Connection,
    thread_id: str,
    role: str,
    suffix: str,
    rollout_path: Path,
) -> None:
    state.execute(
        "INSERT INTO threads VALUES (?, ?, ?, ?, ?, ?)",
        (thread_id, "/workspace/codex", "cli", role, role, str(rollout_path)),
    )
    _write_json_lines(
        path=rollout_path,
        records=[
            {
                "type": "turn_context",
                "payload": {
                    "turn_id": "turn-0" + suffix,
                    "workspace_roots": ["/workspace/codex/before"],
                    "cwd": "/workspace/codex/before",
                },
            },
            {
                "type": "turn_context",
                "payload": {
                    "turn_id": "turn-1" + suffix,
                    "workspace_roots": [],
                    "cwd": "/workspace/codex/next",
                },
            },
            {
                "type": "turn_context",
                "payload": {
                    "turn_id": "turn-2" + suffix,
                    "workspace_roots": ["/workspace/codex"],
                    "cwd": "/workspace/codex",
                },
            },
        ],
    )
    _insert_codex_item(
        connection=history,
        thread_id=thread_id,
        turn_id="turn-0" + suffix,
        ordinal=0,
        timestamp=1767225600000,
        item={
            "type": "userMessage",
            "content": [{"type": "text", "text": "initial"}],
        },
    )
    _insert_codex_item(
        connection=history,
        thread_id=thread_id,
        turn_id="turn-0" + suffix,
        ordinal=1,
        timestamp=1767312000000,
        item={
            "type": "fileChange",
            "status": "completed",
            "changes": [
                {
                    "path": "/workspace/codex/before/example.py",
                    "diff": "-before\n+intermediate",
                },
                {"path": "/workspace/codex/next/private.py", "diff": "+private"},
            ],
        },
    )
    _insert_codex_item(
        connection=history,
        thread_id=thread_id,
        turn_id="turn-1" + suffix,
        ordinal=2,
        timestamp=1767398400000,
        item={
            "type": "userMessage",
            "content": [{"type": "text", "text": "Keep the public name"}],
        },
    )
    _insert_codex_item(
        connection=history,
        thread_id=thread_id,
        turn_id="turn-1" + suffix,
        ordinal=3,
        timestamp=1767484800000,
        item={
            "type": "fileChange",
            "status": "completed",
            "changes": [
                {
                    "path": "/workspace/codex/next/example.py",
                    "diff": "-intermediate\n+after",
                },
                {"path": "/workspace/codex/notes.md", "diff": "+notes"},
                {"path": "/private/secret.py", "diff": "+secret"},
            ],
        },
    )
    _insert_codex_item(
        connection=history,
        thread_id=thread_id,
        turn_id="turn-2" + suffix,
        ordinal=4,
        timestamp=1767571200000,
        item={
            "type": "userMessage",
            "content": [{"type": "text", "text": "thanks"}],
        },
    )


def _create_codex_history(home: Path) -> None:
    home.mkdir(parents=True)
    with sqlite3.connect(str(home / "state_5.sqlite")) as state:
        state.execute(
            "CREATE TABLE threads "
            "(id TEXT, cwd TEXT, source TEXT, agent_role TEXT, thread_source TEXT, "
            "rollout_path TEXT)"
        )
        with sqlite3.connect(str(home / "thread_history_1.sqlite")) as history:
            history.execute(
                "CREATE TABLE thread_items "
                "(thread_id TEXT, turn_id TEXT, rollout_ordinal INTEGER, "
                "created_at_ms INTEGER, item_type TEXT, item_json TEXT)"
            )
            _add_codex_sandwich(
                state=state,
                history=history,
                thread_id="codex-main",
                role="user",
                suffix="",
                rollout_path=home / "codex-main.jsonl",
            )
            _add_codex_sandwich(
                state=state,
                history=history,
                thread_id="codex-subagent",
                role="subagent",
                suffix="-sub",
                rollout_path=home / "codex-subagent.jsonl",
            )
            _add_codex_sandwich(
                state=state,
                history=history,
                thread_id="codex-voice",
                role="realtime_voice",
                suffix="-voice",
                rollout_path=home / "codex-voice.jsonl",
            )
            state.execute(
                "INSERT INTO threads VALUES (?, ?, ?, ?, ?, ?)",
                (
                    "codex-non-python",
                    "/workspace/codex",
                    "cli",
                    "user",
                    "user",
                    str(home / "codex-non-python.jsonl"),
                ),
            )
            _write_json_lines(
                path=home / "codex-non-python.jsonl",
                records=[
                    {
                        "type": "turn_context",
                        "payload": {
                            "turn_id": "non-python-%d" % ordinal,
                            "workspace_roots": ["/workspace/codex"],
                            "cwd": "/workspace/codex",
                        },
                    }
                    for ordinal in range(4)
                ],
            )
            for ordinal, item in enumerate(
                [
                    {
                        "type": "userMessage",
                        "content": [{"type": "text", "text": "initial"}],
                    },
                    {
                        "type": "fileChange",
                        "status": "completed",
                        "changes": [
                            {"path": "/workspace/codex/a.md", "diff": "+before"},
                            {"path": "/workspace/codex/a.PY", "diff": "+before"},
                        ],
                    },
                    {
                        "type": "userMessage",
                        "content": [{"type": "text", "text": "feedback"}],
                    },
                    {
                        "type": "fileChange",
                        "status": "completed",
                        "changes": [
                            {"path": "/workspace/codex/a.md", "diff": "+after"},
                            {"path": "/workspace/codex/a.PY", "diff": "+after"},
                        ],
                    },
                ]
            ):
                _insert_codex_item(
                    connection=history,
                    thread_id="codex-non-python",
                    turn_id="non-python-%d" % ordinal,
                    ordinal=ordinal,
                    timestamp=1767225600000 + ordinal,
                    item=item,
                )


def _write_json_lines(path: Path, records: Sequence[Dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(record) + "\n" for record in records), encoding="utf-8"
    )


def _create_claude_history(home: Path) -> None:
    project = home / "projects" / "synthetic"
    records = [
        {
            "type": "user",
            "uuid": "claude-turn-0",
            "timestamp": "2026-01-01T00:00:00Z",
            "cwd": "/workspace/claude",
            "promptSource": "typed",
            "message": {"role": "user", "content": "initial"},
        },
        {
            "type": "assistant",
            "uuid": "claude-mutation-0",
            "timestamp": "2026-01-02T00:00:00Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "write-module",
                        "name": "Write",
                        "input": {
                            "file_path": "/workspace/claude/module.py",
                            "content": "value = 1\n",
                        },
                    }
                ],
            },
        },
        {
            "type": "user",
            "uuid": "claude-generated",
            "timestamp": "2026-01-02T12:00:00Z",
            "cwd": "/workspace/claude",
            "promptSource": "sdk",
            "origin": {"kind": "human"},
            "message": {"role": "user", "content": "generated SDK prompt"},
        },
        {
            "type": "user",
            "uuid": "claude-origin-generated",
            "timestamp": "2026-01-02T12:15:00Z",
            "cwd": "/workspace/claude",
            "origin": {"kind": "task-notification"},
            "message": {"role": "user", "content": "generated notification"},
        },
        {
            "type": "user",
            "uuid": "claude-legacy-generated",
            "timestamp": "2026-01-02T12:30:00Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "user",
                "content": "<local-command-caveat>generated</local-command-caveat>",
            },
        },
        {
            "type": "user",
            "uuid": "claude-unknown",
            "timestamp": "2026-01-02T12:45:00Z",
            "cwd": "/workspace/claude",
            "message": {"role": "user", "content": "unmarked generated prompt"},
        },
        {
            "type": "user",
            "uuid": "claude-compaction",
            "timestamp": "2026-01-02T13:00:00Z",
            "cwd": "/workspace/claude",
            "isCompactSummary": True,
            "isVisibleInTranscriptOnly": True,
            "message": {"role": "user", "content": "generated summary"},
        },
        {
            "type": "user",
            "uuid": "claude-write-result",
            "timestamp": "2026-01-02T00:00:01Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "write-module",
                        "content": "ok",
                    }
                ],
            },
            "toolUseResult": {
                "type": "create",
                "structuredPatch": [],
            },
        },
        {
            "type": "user",
            "uuid": "claude-meta",
            "timestamp": "2026-01-03T00:00:00Z",
            "cwd": "/workspace/claude",
            "isMeta": True,
            "message": {"role": "user", "content": "generated metadata"},
        },
        {
            "type": "user",
            "uuid": "claude-tool-result",
            "timestamp": "2026-01-03T12:00:00Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "other-tool",
                        "content": "ok",
                    }
                ],
            },
        },
        {
            "type": "user",
            "uuid": "claude-turn-1",
            "timestamp": "2026-01-04T00:00:00Z",
            "cwd": "/workspace/claude",
            "origin": {"kind": "human"},
            "message": {
                "role": "user",
                "content": [{"type": "text", "text": "Use the clearer value"}],
            },
        },
        {
            "type": "assistant",
            "uuid": "claude-mutation-1",
            "timestamp": "2026-01-05T00:00:00Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "edit-module",
                        "name": "Edit",
                        "input": {
                            "file_path": "/workspace/claude/module.py",
                            "old_string": "value",
                            "new_string": "result",
                            "replace_all": True,
                        },
                    },
                    {
                        "type": "tool_use",
                        "id": "write-outside",
                        "name": "Write",
                        "input": {
                            "file_path": "/outside/private.py",
                            "content": "secret = True\n",
                        },
                    },
                    {
                        "type": "tool_use",
                        "id": "write-noop",
                        "name": "Write",
                        "input": {
                            "file_path": "/workspace/claude/noop.py",
                            "content": "same = True\n",
                        },
                    },
                ],
            },
        },
        {
            "type": "user",
            "uuid": "claude-edit-result",
            "timestamp": "2026-01-05T00:00:01Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "edit-module",
                        "content": "ok",
                    }
                ],
            },
            "toolUseResult": {
                "structuredPatch": [
                    {
                        "oldStart": 1,
                        "oldLines": 1,
                        "newStart": 1,
                        "newLines": 1,
                        "lines": ["-value = 1", "+result = 1"],
                    },
                    {
                        "oldStart": 3,
                        "oldLines": 1,
                        "newStart": 3,
                        "newLines": 1,
                        "lines": ["-value = 2", "+result = 2"],
                    },
                ]
            },
        },
        {
            "type": "user",
            "uuid": "claude-outside-result",
            "timestamp": "2026-01-05T00:00:02Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "write-outside",
                        "is_error": True,
                        "content": "failed",
                    }
                ],
            },
        },
        {
            "type": "user",
            "uuid": "claude-noop-result",
            "timestamp": "2026-01-05T00:00:03Z",
            "cwd": "/workspace/claude",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "write-noop",
                        "content": "ok",
                    }
                ],
            },
            "toolUseResult": {
                "type": "update",
                "structuredPatch": [],
            },
        },
        {
            "type": "user",
            "uuid": "claude-turn-2",
            "timestamp": "2026-01-06T00:00:00Z",
            "cwd": "/workspace/claude",
            "origin": {"kind": "human"},
            "message": {"role": "user", "content": "thanks"},
        },
    ]
    records = [dict(record, sessionId="claude-session") for record in records]
    _write_json_lines(path=project / "claude-main.jsonl", records=records)
    _write_json_lines(path=project / "claude-main.orphaned-123.jsonl", records=records)
    sidechain = [dict(record, isSidechain=True) for record in records]
    _write_json_lines(path=project / "claude-sidechain.jsonl", records=sidechain)
    _write_json_lines(
        path=project / "subagents" / "claude-child.jsonl", records=sidechain
    )


def _get_tool_path() -> Path:
    return Path(__file__).parents[2] / "tools" / "review_miner.py"


def _run_miner(
    root: Path, extra_arguments: Sequence[str]
) -> Tuple[subprocess.CompletedProcess, Path]:
    result = subprocess.run(
        args=[
            sys.executable,
            str(_get_tool_path()),
            "--codex-home",
            str(root / "codex"),
            "--claude-home",
            str(root / "claude"),
        ]
        + list(extra_arguments),
        capture_output=True,
        text=True,
    )
    path = Path(result.stdout.strip()) if result.stdout.strip() else Path()
    return result, path


def test_extracts_bounded_common_episodes() -> None:
    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        _create_codex_history(home=root / "codex")
        _create_claude_history(home=root / "claude")

        result, path = _run_miner(
            root=root,
            extra_arguments=[
                "--since",
                "2026-01-01T00:00:00Z",
                "--limit",
                "10",
            ],
        )
        assert result.returncode == 0, result.stderr
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        report = json.loads(path.read_text(encoding="utf-8"))
        path.unlink()

        episodes = report["episodes"]
        assert [episode["source"] for episode in episodes] == ["claude", "codex"]
        assert episodes[0]["thread_id"] == "claude-session"
        assert episodes[0]["turn_id"] == "claude-turn-1"
        assert episodes[0]["paths"] == ["module.py"]
        assert episodes[0]["feedback"] == "Use the clearer value"
        assert "--- /dev/null" in episodes[0]["before"]["changes"][0]["diff"]
        assert "+value = 1" in episodes[0]["before"]["changes"][0]["diff"]
        assert "--- a/module.py" in episodes[0]["after"]["changes"][0]["diff"]
        assert "+result = 2" in episodes[0]["after"]["changes"][0]["diff"]
        assert episodes[1]["turn_id"] == "turn-1"
        assert episodes[1]["paths"] == ["example.py"]
        assert episodes[1]["feedback"] == "Keep the public name"
        assert (
            "claude: skipped 1 user turns without explicit human provenance"
            in result.stderr
        )

        result, path = _run_miner(
            root=root,
            extra_arguments=[
                "--since",
                "2026-01-03T12:00:00Z",
                "--limit",
                "1",
            ],
        )
        assert result.returncode == 0, result.stderr
        report = json.loads(path.read_text(encoding="utf-8"))
        path.unlink()
        assert [episode["source"] for episode in report["episodes"]] == ["claude"]

        special_codex_home = root / "codex?special"
        _create_codex_history(home=special_codex_home)
        result, path = _run_miner(
            root=root,
            extra_arguments=[
                "--codex-home",
                str(special_codex_home),
                "--claude-home",
                str(root / "missing-claude"),
                "--since",
                "2026-01-01T00:00:00Z",
            ],
        )
        assert result.returncode == 0, result.stderr
        path.unlink()


def test_reports_missing_and_incompatible_sources() -> None:
    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        _create_claude_history(home=root / "claude")
        result, path = _run_miner(
            root=root, extra_arguments=["--since", "2026-01-01T00:00:00Z"]
        )
        assert result.returncode == 0
        assert "codex: history source is missing" in result.stderr
        path.unlink()

    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        result, _ = _run_miner(
            root=root, extra_arguments=["--since", "2026-01-01T00:00:00Z"]
        )
        assert result.returncode == 2
        assert "no compatible history source is available" in result.stderr

    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        codex = root / "codex"
        codex.mkdir()
        sqlite3.connect(str(codex / "state_5.sqlite")).close()
        sqlite3.connect(str(codex / "thread_history_1.sqlite")).close()
        _create_claude_history(home=root / "claude")
        result, path = _run_miner(
            root=root, extra_arguments=["--since", "2026-01-01T00:00:00Z"]
        )
        assert result.returncode == 0
        assert "codex: incompatible history schema" in result.stderr
        path.unlink()

    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        _create_codex_history(home=root / "codex")
        transcript = root / "claude" / "projects" / "synthetic" / "bad.jsonl"
        transcript.parent.mkdir(parents=True)
        transcript.write_text("{\n", encoding="utf-8")
        result, path = _run_miner(
            root=root, extra_arguments=["--since", "2026-01-01T00:00:00Z"]
        )
        assert result.returncode == 0
        assert "%s: line 1: invalid JSON" % transcript in result.stderr
        path.unlink()

    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        _create_codex_history(home=root / "codex")
        _create_claude_history(home=root / "claude")
        transcript = root / "claude" / "projects" / "synthetic" / "claude-main.jsonl"
        records = [
            json.loads(line)
            for line in transcript.read_text(encoding="utf-8").splitlines()
        ]
        write_result = next(
            record for record in records if record.get("uuid") == "claude-write-result"
        )
        write_result["toolUseResult"]["structuredPatch"] = [
            {
                "oldStart": 1,
                "oldLines": 0,
                "newStart": 1,
                "newLines": 0,
                "lines": ["+value = 1"],
            }
        ]
        _write_json_lines(path=transcript, records=records)

        result, path = _run_miner(
            root=root, extra_arguments=["--since", "2026-01-01T00:00:00Z"]
        )
        assert result.returncode == 0
        assert "claude: incompatible history schema" in result.stderr
        assert "structured patch has inconsistent line counts" in result.stderr
        report = json.loads(path.read_text(encoding="utf-8"))
        path.unlink()
        assert [episode["source"] for episode in report["episodes"]] == ["codex"]


def test_removes_failed_reports() -> None:
    with tempfile.TemporaryDirectory() as raw_root:
        root = Path(raw_root)
        _create_claude_history(home=root / "claude")
        report_directory = root / "reports"
        report_directory.mkdir()
        environment = dict(os.environ, TMPDIR=str(report_directory))
        process = subprocess.Popen(
            args=[
                sys.executable,
                str(_get_tool_path()),
                "--codex-home",
                str(root / "codex"),
                "--claude-home",
                str(root / "claude"),
                "--since",
                "2026-01-01T00:00:00Z",
            ],
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        assert process.stdout is not None
        assert process.stderr is not None
        process.stdout.close()
        process.stderr.read()
        assert process.wait() != 0
        assert not list(report_directory.glob("ruffhouse-review-*.json"))

        hook_directory = root / "hook"
        hook_directory.mkdir()
        hook_directory.joinpath("sitecustomize.py").write_text(
            "import json\n"
            "\n"
            "def fail_dump(*args, **kwargs):\n"
            "    raise OSError('synthetic write failure')\n"
            "\n"
            "json.dump = fail_dump\n",
            encoding="utf-8",
        )
        environment = dict(
            os.environ,
            PYTHONPATH=str(hook_directory),
            TMPDIR=str(report_directory),
        )
        result = subprocess.run(
            args=[
                sys.executable,
                str(_get_tool_path()),
                "--codex-home",
                str(root / "codex"),
                "--claude-home",
                str(root / "claude"),
                "--since",
                "2026-01-01T00:00:00Z",
            ],
            capture_output=True,
            env=environment,
            text=True,
        )
        assert result.returncode != 0
        assert not list(report_directory.glob("ruffhouse-review-*.json"))


if __name__ == "__main__":
    test_extracts_bounded_common_episodes()
    test_reports_missing_and_incompatible_sources()
    test_removes_failed_reports()
