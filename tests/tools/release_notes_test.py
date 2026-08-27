from tools.release_notes import extract_release_notes


def test_extract_release_notes() -> None:
    changelog = """# Changelog

## [Unreleased]

## [0.0.1] - 2026-08-27

### Added

- Added Gruff.

[0.0.1]: https://github.com/wkentaro/gruff/releases/tag/v0.0.1
"""

    assert extract_release_notes(changelog=changelog, tag="v0.0.1") == (
        "### Added\n\n- Added Gruff.\n"
    )


def test_reject_missing_release() -> None:
    try:
        extract_release_notes(changelog="# Changelog\n", tag="v0.0.1")
    except ValueError as error:
        assert str(error) == "No CHANGELOG.md section found for 0.0.1"
    else:
        raise AssertionError("Missing release was accepted")


if __name__ == "__main__":
    test_extract_release_notes()
    test_reject_missing_release()
