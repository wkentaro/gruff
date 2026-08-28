import argparse
from pathlib import Path


def extract_release_notes(*, changelog: str, tag: str) -> str:
    version = tag[1:] if tag.startswith("v") else tag
    heading = f"## [{version}]"
    lines = changelog.splitlines(keepends=True)
    try:
        start = next(
            index + 1
            for index, line in enumerate(lines)
            if line.rstrip() == heading or line.startswith(f"{heading} - ")
        )
    except StopIteration as error:
        raise ValueError(f"No CHANGELOG.md section found for {version}") from error

    end = next(
        (
            index
            for index in range(start, len(lines))
            if lines[index].startswith("## [") or lines[index].startswith("[")
        ),
        len(lines),
    )
    notes = "".join(lines[start:end]).strip()
    if not notes:
        raise ValueError(f"CHANGELOG.md section for {version} is empty")
    return f"{notes}\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("tag")
    parser.add_argument("changelog", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    try:
        notes = extract_release_notes(
            changelog=args.changelog.read_text(encoding="utf-8"),
            tag=args.tag,
        )
    except ValueError as error:
        parser.error(str(error))

    args.output.write_text(notes, encoding="utf-8")


if __name__ == "__main__":
    main()
