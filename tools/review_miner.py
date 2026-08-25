#!/usr/bin/env python3

import argparse
import difflib
import json
import os
import sqlite3
import sys
import tempfile
from datetime import datetime
from datetime import timedelta
from datetime import timezone
from pathlib import Path
from typing import Any
from typing import Dict
from typing import List
from typing import Optional
from typing import Sequence
from typing import Tuple


def _parse_timestamp(value: str) -> datetime:
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    timestamp = datetime.fromisoformat(normalized)
    if timestamp.tzinfo is None:
        timestamp = timestamp.replace(tzinfo=timezone.utc)
    return timestamp.astimezone(timezone.utc)


def _format_timestamp(timestamp: datetime) -> str:
    return timestamp.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _read_workspace_roots(record: Dict[str, Any], fallback: str) -> List[str]:
    roots = record.get("workspace_roots")
    if roots is None or roots == []:
        roots = [fallback]
    if not isinstance(roots, list) or not all(isinstance(root, str) for root in roots):
        raise ValueError("workspace roots are not a list of paths")
    if not roots or not all(roots):
        raise ValueError("no workspace root or working directory was recorded")
    return roots


def _require_columns(
    connection: sqlite3.Connection, table: str, required: Sequence[str]
) -> None:
    columns = {row[1] for row in connection.execute("PRAGMA table_info(" + table + ")")}
    missing = set(required) - columns
    if missing:
        raise ValueError(
            "%s is missing columns: %s" % (table, ", ".join(sorted(missing)))
        )


def _connect_read_only(path: Path) -> sqlite3.Connection:
    return sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True)


def _join_text_parts(content: Sequence[Any]) -> Optional[str]:
    text = "\n".join(
        part["text"]
        for part in content
        if isinstance(part, dict)
        and part.get("type") == "text"
        and isinstance(part.get("text"), str)
    ).strip()
    return text or None


def _read_codex_text(item: Dict[str, Any]) -> Optional[str]:
    content = item.get("content")
    if not isinstance(content, list):
        raise ValueError("a user message has no content list")
    return _join_text_parts(content=content)


def _read_codex_changes(item: Dict[str, Any]) -> List[Dict[str, str]]:
    if item.get("status") != "completed":
        return []
    changes = item.get("changes")
    if not isinstance(changes, list):
        raise ValueError("a completed file change has no changes list")
    result = []
    for change in changes:
        if not isinstance(change, dict):
            raise ValueError("a file change entry is not an object")
        path = change.get("path")
        diff = change.get("diff")
        if not isinstance(path, str) or not isinstance(diff, str):
            raise ValueError("a file change entry has no path or diff")
        result.append({"path": path, "diff": diff})
    return result


def _read_codex_turn_roots(path: Path) -> Dict[str, List[str]]:
    roots_by_turn = {}
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise ValueError(
                    "%s:%d: invalid JSON: %s" % (path, line_number, error.msg)
                )
            if not isinstance(record, dict) or record.get("type") != "turn_context":
                continue
            payload = record.get("payload")
            if not isinstance(payload, dict) or not isinstance(
                payload.get("turn_id"), str
            ):
                raise ValueError("%s:%d: invalid turn context" % (path, line_number))
            roots_by_turn[payload["turn_id"]] = _read_workspace_roots(
                record=payload, fallback=payload.get("cwd", "")
            )
    return roots_by_turn


def _read_codex_threads(home: Path) -> Tuple[bool, List[Dict[str, Any]], str]:
    state_path = home / "state_5.sqlite"
    history_path = home / "thread_history_1.sqlite"
    if not state_path.is_file() or not history_path.is_file():
        return False, [], "codex: history source is missing"

    try:
        with _connect_read_only(state_path) as state:
            _require_columns(
                connection=state,
                table="threads",
                required=["id", "rollout_path", "thread_source"],
            )
            roots_by_thread = {}
            for thread_id, rollout_path in state.execute(
                "SELECT id, rollout_path FROM threads WHERE thread_source = 'user'"
            ):
                roots_by_thread[thread_id] = _read_codex_turn_roots(
                    path=Path(rollout_path)
                )

        with _connect_read_only(history_path) as history:
            _require_columns(
                connection=history,
                table="thread_items",
                required=[
                    "thread_id",
                    "turn_id",
                    "rollout_ordinal",
                    "created_at_ms",
                    "item_json",
                    "item_type",
                ],
            )
            threads = {}
            rows = history.execute(
                "SELECT thread_id, turn_id, created_at_ms, "
                "item_type, item_json FROM thread_items "
                "WHERE item_type IN ('userMessage', 'fileChange') "
                "ORDER BY thread_id, rollout_ordinal"
            )
            missing_contexts = set()
            for thread_id, turn_id, created_at_ms, item_type, raw in rows:
                roots_by_turn = roots_by_thread.get(thread_id)
                if roots_by_turn is None:
                    continue
                roots = roots_by_turn.get(turn_id)
                if roots is None:
                    missing_contexts.add((thread_id, turn_id))
                    continue
                item = json.loads(raw)
                if not isinstance(item, dict) or item.get("type") != item_type:
                    raise ValueError("a normalized item does not match its item type")
                event = {
                    "thread_id": thread_id,
                    "turn_id": turn_id,
                    "timestamp": _format_timestamp(
                        timestamp=datetime.fromtimestamp(
                            created_at_ms / 1000, tz=timezone.utc
                        )
                    ),
                }
                if item_type == "userMessage":
                    text = _read_codex_text(item)
                    if text is None:
                        continue
                    event.update(
                        {
                            "kind": "human",
                            "text": text,
                        }
                    )
                else:
                    changes = _read_codex_changes(item)
                    if not changes:
                        continue
                    event.update(
                        {"kind": "mutation", "changes": changes, "roots": roots}
                    )
                threads.setdefault(thread_id, []).append(event)
        diagnostic = ""
        if missing_contexts:
            diagnostic = "codex: skipped %d turns without workspace context" % len(
                missing_contexts
            )
        return True, list(threads.values()), diagnostic
    except (
        json.JSONDecodeError,
        OSError,
        sqlite3.DatabaseError,
        TypeError,
        ValueError,
    ) as error:
        return False, [], "codex: incompatible history schema: %s" % error


def _read_claude_text(record: Dict[str, Any]) -> Optional[str]:
    message = record.get("message")
    if not isinstance(message, dict) or message.get("role") != "user":
        raise ValueError("a user event has no user message")
    content = message.get("content")
    if isinstance(content, str):
        return content.strip() or None
    if not isinstance(content, list):
        raise ValueError("a user message has unsupported content")
    if any(
        isinstance(part, dict) and part.get("type") == "tool_result" for part in content
    ):
        return None
    return _join_text_parts(content=content)


def _classify_direct_claude_user(record: Dict[str, Any], text: str) -> Optional[bool]:
    if (
        record.get("isMeta") is True
        or record.get("isCompactSummary") is True
        or record.get("isVisibleInTranscriptOnly") is True
    ):
        return False
    prompt_source = record.get("promptSource")
    if prompt_source is not None and not isinstance(prompt_source, str):
        raise ValueError("a user event has unsupported prompt source metadata")
    origin = record.get("origin")
    if origin is not None:
        if not isinstance(origin, dict) or not isinstance(origin.get("kind"), str):
            raise ValueError("a user event has unsupported origin metadata")
    if text.startswith(
        (
            "<bash-input>",
            "<bash-stdout>",
            "<command-message>",
            "<command-name>",
            "<local-command-caveat>",
            "<local-command-stdout>",
            "<system-reminder>",
            "<task-notification>",
        )
    ):
        return False
    if prompt_source in {"sdk", "system"} or (
        origin is not None and origin["kind"] != "human"
    ):
        return False
    if (origin is not None and origin["kind"] == "human") or prompt_source in {
        "queued",
        "suggestion_accepted",
        "typed",
    }:
        return True
    return None


def _read_successful_claude_results(
    records: Sequence[Dict[str, Any]],
) -> Dict[str, Dict[str, Any]]:
    mutation_tool_ids = set()
    for record in records:
        message = record.get("message")
        if not isinstance(message, dict) or message.get("role") != "assistant":
            continue
        content = message.get("content")
        if not isinstance(content, list):
            continue
        for part in content:
            if (
                isinstance(part, dict)
                and part.get("type") == "tool_use"
                and part.get("name") in {"Edit", "Write"}
            ):
                tool_id = part.get("id")
                if not isinstance(tool_id, str):
                    raise ValueError("a mutation has no tool identifier")
                mutation_tool_ids.add(tool_id)

    successful = {}
    for record in records:
        message = record.get("message")
        if not isinstance(message, dict) or message.get("role") != "user":
            continue
        content = message.get("content")
        if not isinstance(content, list):
            continue
        results = [
            part
            for part in content
            if isinstance(part, dict)
            and part.get("type") == "tool_result"
            and part.get("tool_use_id") in mutation_tool_ids
        ]
        if not results:
            continue
        if len(results) != 1:
            raise ValueError("a mutation result record contains multiple results")
        result = results[0]
        if result.get("is_error") is True:
            continue
        tool_result = record.get("toolUseResult")
        if not isinstance(tool_result, dict) or not isinstance(
            tool_result.get("structuredPatch"), list
        ):
            raise ValueError("a successful mutation has no structured patch")
        for hunk in tool_result["structuredPatch"]:
            if not isinstance(hunk, dict) or not all(
                type(hunk.get(key)) is int and hunk[key] >= 0
                for key in ("oldStart", "oldLines", "newStart", "newLines")
            ):
                raise ValueError("a structured patch has invalid line ranges")
            lines = hunk.get("lines")
            if not isinstance(lines, list) or not all(
                isinstance(line, str)
                and line.startswith((" ", "+", "-", "\\ No newline at end of file"))
                for line in lines
            ):
                raise ValueError("a structured patch has invalid lines")
            old_lines = sum(line.startswith((" ", "-")) for line in lines)
            new_lines = sum(line.startswith((" ", "+")) for line in lines)
            if old_lines != hunk["oldLines"] or new_lines != hunk["newLines"]:
                raise ValueError("a structured patch has inconsistent line counts")
        successful[result["tool_use_id"]] = tool_result
    return successful


def _read_claude_changes(
    record: Dict[str, Any], successful_results: Dict[str, Dict[str, Any]]
) -> List[Dict[str, Any]]:
    message = record.get("message")
    if not isinstance(message, dict) or message.get("role") != "assistant":
        raise ValueError("an assistant event has no assistant message")
    content = message.get("content")
    if not isinstance(content, list):
        raise ValueError("an assistant message has unsupported content")
    changes = []
    for part in content:
        if not isinstance(part, dict) or part.get("type") != "tool_use":
            continue
        name = part.get("name")
        if name not in {"Edit", "Write"}:
            continue
        tool_id = part.get("id")
        if not isinstance(tool_id, str):
            raise ValueError("a %s mutation has no tool identifier" % name)
        tool_result = successful_results.get(tool_id)
        if tool_result is None:
            continue
        arguments = part.get("input")
        if not isinstance(arguments, dict) or not isinstance(
            arguments.get("file_path"), str
        ):
            raise ValueError("a %s mutation has no file path" % name)
        change = {
            "path": arguments["file_path"],
            "hunks": tool_result["structuredPatch"],
        }
        if not change["hunks"]:
            if name != "Write" or tool_result.get("type") != "create":
                continue
            content = arguments.get("content")
            if not isinstance(content, str):
                raise ValueError("a Write mutation has no content")
            change["content"] = content
        changes.append(change)
    return changes


def _read_claude_transcript(
    path: Path,
) -> Tuple[List[Dict[str, Any]], int]:
    records = []
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise ValueError("line %d: invalid JSON: %s" % (line_number, error.msg))
            if not isinstance(record, dict):
                raise ValueError(
                    "line %d: transcript record is not an object" % line_number
                )
            records.append(record)
    if any(record.get("isSidechain") is True for record in records):
        return [], 0

    session_id = None
    for record in records:
        if record.get("type") not in {"user", "assistant"}:
            continue
        current_session_id = record.get("sessionId")
        if not isinstance(current_session_id, str):
            raise ValueError("a transcript turn has no session identifier")
        if session_id is not None and current_session_id != session_id:
            raise ValueError("a transcript contains multiple session identifiers")
        session_id = current_session_id
    if session_id is None:
        return [], 0

    events = []
    skipped_users = 0
    successful_results = _read_successful_claude_results(records)
    for record in records:
        event_type = record.get("type")
        if event_type == "user":
            text = _read_claude_text(record)
            if text is None:
                continue
            classification = _classify_direct_claude_user(record=record, text=text)
            if classification is None:
                skipped_users += 1
                continue
            if not classification:
                continue
            event = {
                "kind": "human",
                "text": text,
            }
        elif event_type == "assistant":
            changes = _read_claude_changes(
                record=record, successful_results=successful_results
            )
            if not changes:
                continue
            event = {
                "kind": "mutation",
                "changes": changes,
                "roots": _read_workspace_roots(
                    record=record, fallback=record.get("cwd", "")
                ),
            }
        else:
            continue
        turn_id = record.get("uuid")
        timestamp = record.get("timestamp")
        if not isinstance(turn_id, str) or not isinstance(timestamp, str):
            raise ValueError("a transcript turn has no identifier or timestamp")
        event.update(
            {
                "thread_id": session_id,
                "turn_id": turn_id,
                "timestamp": _format_timestamp(
                    timestamp=_parse_timestamp(value=timestamp)
                ),
            }
        )
        events.append(event)
    return events, skipped_users


def _read_claude_threads(home: Path) -> Tuple[bool, List[Dict[str, Any]], str]:
    projects = home / "projects"
    if not projects.is_dir():
        return False, [], "claude: history source is missing"

    try:
        threads = []
        skipped_users = 0
        for path in sorted(projects.rglob("*.jsonl")):
            if (
                "subagents" in path.relative_to(projects).parts
                or ".orphaned-" in path.name
            ):
                continue
            try:
                events, transcript_skipped_users = _read_claude_transcript(path=path)
            except (OSError, TypeError, ValueError) as error:
                raise ValueError("%s: %s" % (path, error))
            skipped_users += transcript_skipped_users
            if events:
                threads.append(events)
        diagnostic = ""
        if skipped_users:
            diagnostic = (
                "claude: skipped %d user turns without explicit human provenance"
                % skipped_users
            )
        return True, threads, diagnostic
    except (OSError, TypeError, ValueError) as error:
        return False, [], "claude: incompatible history schema: %s" % error


def _make_relative_path(path: str, roots: Sequence[str]) -> Optional[str]:
    source = Path(path)
    for root_value in roots:
        root = Path(root_value).resolve()
        candidate = (
            source.resolve() if source.is_absolute() else (root / source).resolve()
        )
        try:
            relative = candidate.relative_to(root)
        except ValueError:
            continue
        if relative.suffix in {".py", ".pyi", ".pyw"}:
            return relative.as_posix()
    return None


def _render_diff(change: Dict[str, Any], path: str) -> str:
    if "diff" in change:
        return change["diff"]
    if "content" in change:
        return "\n".join(
            difflib.unified_diff(
                [],
                change["content"].splitlines(),
                fromfile="/dev/null",
                tofile="b/" + path,
                lineterm="",
            )
        )
    lines = ["--- a/" + path, "+++ b/" + path]
    for hunk in change["hunks"]:
        lines.append(
            "@@ -%d,%d +%d,%d @@"
            % (
                hunk["oldStart"],
                hunk["oldLines"],
                hunk["newStart"],
                hunk["newLines"],
            )
        )
        lines.extend(hunk["lines"])
    return "\n".join(lines)


def _filter_changes(
    changes: Sequence[Dict[str, Any]], roots: Sequence[str]
) -> List[Dict[str, str]]:
    result = []
    for change in changes:
        path = _make_relative_path(path=change["path"], roots=roots)
        if path is None:
            continue
        diff = _render_diff(change=change, path=path)
        if diff:
            result.append({"path": path, "diff": diff})
    return result


def _make_mutation(event: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    changes = _filter_changes(changes=event["changes"], roots=event["roots"])
    if not changes:
        return None
    return {
        "turn_id": event["turn_id"],
        "timestamp": event["timestamp"],
        "changes": changes,
    }


def _extract_episodes(
    source: str, threads: Sequence[List[Dict[str, Any]]], since: datetime
) -> List[Dict[str, Any]]:
    episodes = []
    for events in threads:
        human_count = 0
        for index, event in enumerate(events):
            if event["kind"] != "human":
                continue
            human_count += 1
            if human_count == 1 or _parse_timestamp(value=event["timestamp"]) < since:
                continue

            # ponytail: scan nearby events; index mutations if very long sessions become slow.
            before = next(
                (
                    mutation
                    for candidate in reversed(events[:index])
                    if candidate["kind"] == "mutation"
                    for mutation in [_make_mutation(event=candidate)]
                    if mutation is not None
                ),
                None,
            )
            after = next(
                (
                    mutation
                    for candidate in events[index + 1 :]
                    if candidate["kind"] == "mutation"
                    for mutation in [_make_mutation(event=candidate)]
                    if mutation is not None
                ),
                None,
            )
            if before is None or after is None:
                continue
            paths = sorted(
                {change["path"] for change in before["changes"] + after["changes"]}
            )
            episodes.append(
                {
                    "source": source,
                    "thread_id": event["thread_id"],
                    "turn_id": event["turn_id"],
                    "timestamp": event["timestamp"],
                    "paths": paths,
                    "feedback": event["text"],
                    "before": before,
                    "after": after,
                }
            )
    return episodes


def _parse_positive_integer(value: str) -> int:
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return number


def _write_report(episodes: Sequence[Dict[str, Any]]) -> Path:
    descriptor, raw_path = tempfile.mkstemp(prefix="ruffhouse-review-", suffix=".json")
    path = Path(raw_path)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(
                obj={"episodes": episodes},
                fp=stream,
                ensure_ascii=False,
                indent=2,
            )
            stream.write("\n")
    except BaseException:
        try:
            os.close(descriptor)
        except OSError:
            pass
        try:
            path.unlink()
        except FileNotFoundError:
            pass
        raise
    return path


def _mine_review_history(options: argparse.Namespace, since: datetime) -> int:
    sources = [
        ("codex",) + _read_codex_threads(home=options.codex_home),
        ("claude",) + _read_claude_threads(home=options.claude_home),
    ]
    episodes = []
    available = 0
    for source, is_available, threads, diagnostic in sources:
        if diagnostic:
            print(diagnostic, file=sys.stderr)
        if not is_available:
            continue
        available += 1
        episodes.extend(_extract_episodes(source=source, threads=threads, since=since))
    if available == 0:
        print(
            "review miner: no compatible history source is available", file=sys.stderr
        )
        return 2

    episodes.sort(
        key=lambda episode: _parse_timestamp(value=episode["timestamp"]), reverse=True
    )
    path = _write_report(episodes=episodes[: options.limit])
    try:
        print(path, flush=True)
    except BaseException:
        try:
            path.unlink()
        except FileNotFoundError:
            pass
        raise
    return 0


def run(arguments: Optional[Sequence[str]] = None) -> int:
    default_since = _format_timestamp(
        timestamp=datetime.now(timezone.utc) - timedelta(days=30)
    )
    parser = argparse.ArgumentParser(
        description="Extract candidate review episodes from local agent histories."
    )
    parser.add_argument(
        "--since", default=default_since, help="oldest ISO-8601 timestamp"
    )
    parser.add_argument("--limit", type=_parse_positive_integer, default=100)
    parser.add_argument("--codex-home", type=Path, default=Path.home() / ".codex")
    parser.add_argument("--claude-home", type=Path, default=Path.home() / ".claude")
    options = parser.parse_args(arguments)

    try:
        since = _parse_timestamp(value=options.since)
    except ValueError as error:
        parser.error("invalid --since timestamp: %s" % error)
    return _mine_review_history(options=options, since=since)


if __name__ == "__main__":
    sys.exit(run())
