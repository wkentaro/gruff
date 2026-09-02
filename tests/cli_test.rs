use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn place_below_header(source: &str) -> String {
    // GR007 never flags the first five physical lines, so its fixtures start below that floor.
    format!("\n\n\n\n\n{source}")
}

fn create_temp_directory(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gruff-cli-test-{}-{name}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).expect("stale test directory should be removable");
    }
    fs::create_dir(&path).expect("test directory should be created");
    path
}

#[test]
fn describes_check_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--help"])
        .output()
        .expect("gruff should show help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run Gruff on the given files or directories"));
    assert!(
        stdout.contains(
            "--select <RULE_CODE>\n          Comma-separated list of rule codes to enable"
        )
    );
    assert!(stdout.contains("Path to a pyproject.toml configuration file"));
    assert!(stdout.contains("Output serialization format for findings"));
    assert!(output.stderr.is_empty());
}

#[test]
fn checks_config_suppression_json_and_exit_status() {
    let directory = create_temp_directory("config");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff]\noutput-format = \"json\"\n\n[tool.gruff.lint]\nselect = [\"GR001\"]\nper-file-ignores = { \"ignored.py\" = [\"GR001\"], \"invalid.py\" = [\"GR001\"] }\n",
    )
    .expect("test configuration should be written");
    fs::write(directory.join("finding.py"), "def _load(path):\n    ...\n")
        .expect("finding source should be written");
    fs::write(
        directory.join("explicit.py"),
        "def _load(path, /):\n    ...\n\ndef _save(*, path):\n    ...\n",
    )
    .expect("explicit calling conventions should be written");
    fs::write(
        directory.join("suppressed.py"),
        "def _save(path):  # noqa: GR001\n    ...\n",
    )
    .expect("suppressed source should be written");
    fs::write(
        directory.join("noqa_without_codes.py"),
        "def _save(path=None):  # noqa:\n    ...\n",
    )
    .expect("empty code list source should be written");
    fs::write(
        directory.join("url_fragment.py"),
        "def _send(path):  # docs https://example.com/#noqa\n    ...\n",
    )
    .expect("URL fragment source should be written");
    fs::write(
        directory.join("code_list_cutoff.py"),
        "def _save(path):  # noqa: nonsense GR001\n    ...\n",
    )
    .expect("code list cutoff source should be written");
    fs::write(
        directory.join("lowercase_code.py"),
        "def _save(path):  # noqa: gr001\n    ...\n",
    )
    .expect("lowercase code source should be written");
    fs::write(
        directory.join("glued_codes.py"),
        "def _save(path):  # noqa:GR001GR002\n    ...\n",
    )
    .expect("glued code list source should be written");
    fs::write(directory.join("ignored.py"), "def _send(path):\n    ...\n")
        .expect("ignored source should be written");
    fs::write(directory.join("invalid.py"), "def broken(\n")
        .expect("invalid source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    let findings = findings.as_array().unwrap();
    let filenames: Vec<_> = findings
        .iter()
        .map(|finding| {
            PathBuf::from(finding["filename"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    // The suppression-shaped fixtures all fail to suppress: an empty code list names no rule, a
    // URL fragment hash never opens a directive, a code list stops at its first non-code token,
    // joined codes are one unreadable token, and a lowercase code is prose. The remaining files
    // are an unsuppressed GR001 finding and a syntax failure.
    assert_eq!(
        filenames,
        [
            "code_list_cutoff.py",
            "finding.py",
            "glued_codes.py",
            "invalid.py",
            "lowercase_code.py",
            "noqa_without_codes.py",
            "url_fragment.py",
        ]
    );
    let find_by_filename = |name: &str| {
        findings
            .iter()
            .find(|finding| finding["filename"].as_str().unwrap().ends_with(name))
            .unwrap_or_else(|| panic!("{name} should carry a finding"))
    };
    let finding = find_by_filename("finding.py");
    assert_eq!(finding["code"], "GR001");
    assert_eq!(finding["name"], "explicit-non-public-input-conventions");
    assert_eq!(
        finding["message"],
        "Input `path` must be positional-only or keyword-only"
    );
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["location"]["row"], 1);
    assert!(PathBuf::from(finding["filename"].as_str().unwrap()).is_absolute());
    let invalid = find_by_filename("invalid.py");
    assert_eq!(invalid["code"], "invalid-syntax");
    assert_eq!(invalid["name"], "invalid-syntax");
    assert_eq!(invalid["severity"], "error");

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_required_non_public_inputs_selection_and_suppression() {
    let directory = create_temp_directory("required-non-public-inputs");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR002\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(
        directory.join("finding.py"),
        "def _render(*, path=None):\n    ...\n",
    )
    .expect("finding source should be written");
    fs::write(
        directory.join("suppressed.pyi"),
        "def _load(*, path=None): ...  # noqa: GR002\n",
    )
    .expect("suppressed source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["code"], "GR002");
    assert_eq!(findings[0]["name"], "required-non-public-inputs");
    assert_eq!(
        findings[0]["message"],
        "Non-public input `path` must be required"
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_no_non_public_docstrings_selection_and_suppression() {
    let directory = create_temp_directory("no-non-public-docstrings");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff]\noutput-format = \"json\"\n\n[tool.gruff.lint]\nselect = [\"GR006\"]\nper-file-ignores = { \"ignored.py\" = [\"GR006\"] }\n",
    )
    .expect("test configuration should be written");
    fs::write(
        directory.join("finding.py"),
        "def _load():\n    \"\"\"\n    Load.\n    \"\"\"\n",
    )
    .expect("Python finding source should be written");
    fs::write(
        directory.join("finding.pyi"),
        "def __load():\n    \"\"\"Load.\"\"\"\n",
    )
    .expect("stub finding source should be written");
    fs::write(
        directory.join("finding.pyw"),
        "def _write():\n    \"\"\"Write.\"\"\"\n\ndef _load():\n    \"\"\"Load.\"\"\"  # noqa: GR006\n\ndef _save():\n    \"\"\"\n    Save.\n    \"\"\"  # noqa: GR006\n",
    )
    .expect("Python window source should be written");
    fs::write(
        directory.join("ignored.py"),
        "def _send():\n    \"\"\"Send.\"\"\"\n",
    )
    .expect("ignored Python source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 3);
    for finding in findings.as_array().unwrap() {
        assert_eq!(finding["code"], "GR006");
        assert_eq!(finding["name"], "no-non-public-docstrings");
        assert_eq!(finding["location"]["row"], 2);
        assert_eq!(finding["location"]["column"], 5);
    }
    let mut extensions: Vec<_> = findings
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            PathBuf::from(finding["filename"].as_str().unwrap())
                .extension()
                .unwrap()
                .to_owned()
        })
        .collect();
    extensions.sort();
    assert_eq!(extensions, ["py", "pyi", "pyw"]);

    for selector in ["GR", "ALL"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
            .args([
                "check",
                "--isolated",
                "--select",
                selector,
                "--output-format",
                "json",
                "finding.py",
            ])
            .current_dir(&directory)
            .output()
            .expect("gruff should run");
        let findings: Value =
            serde_json::from_slice(&output.stdout).expect("output should be JSON");
        assert_eq!(findings.as_array().unwrap().len(), 1);
        assert_eq!(findings[0]["code"], "GR006");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR006",
            "--ignore",
            "GR006",
            "finding.py",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No rules are enabled"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn flags_no_subsumed_comments_conformance_cases() {
    let directory = create_temp_directory("no-subsumed-comments-findings");
    let mut sources = [
        (
            "get_element.py",
            "async def find_element():\n    # Get the element\n    element = await self.browser_session.get_dom_element_by_index(index)\n",
            (7, 5),
        ),
        (
            "load_examples.py",
            "# Load the examples.\nconfig = _load_examples(config, ...)\n",
            (6, 1),
        ),
        (
            "patch_embedding.py",
            "# Patch embedding\nself.patch_embed = PatchEmbedding(...)\n",
            (6, 1),
        ),
        (
            "return_type.py",
            "# Get return type from last converter.\nrt = _AnnotationExtractor(last).get_return_type()\nif rt:\n    pipe_converter.__annotations__[\"return\"] = rt\n",
            (6, 1),
        ),
        (
            "rounded_rectangle.py",
            "# Draw the rounded rectangle background\ndraw.rounded_rectangle(\n    tuple(box_xyxy),\n    radius=border_radius,\n    fill=background_color,\n)\n",
            (6, 1),
        ),
        (
            "url_components.py",
            "# Test for url components\ndef test_url_with_components():\n    pass\n",
            (6, 1),
        ),
        (
            "vit_backbone.py",
            "# ViT backbone\nvit_backbone = ViT(...)\n",
            (6, 1),
        ),
    ];
    sources.sort_unstable_by_key(|(name, _, _)| *name);
    for (name, source, _) in sources {
        fs::write(directory.join(name), place_below_header(source))
            .expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR007",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), sources.len());
    for (finding, (name, _, (row, column))) in findings.as_array().unwrap().iter().zip(sources) {
        assert!(finding["filename"].as_str().unwrap().ends_with(name));
        assert_eq!(finding["code"], "GR007");
        assert_eq!(finding["name"], "no-subsumed-comments");
        assert_eq!(
            finding["message"],
            "One-line comment restates the statement it annotates; delete it or state what the code cannot"
        );
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
    }

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_no_subsumed_comments_conformance_cases() {
    let directory = create_temp_directory("no-subsumed-comments-allowed");
    let sources = [
        (
            "justified_block.py",
            "# Get the element\n# The cache avoids a second browser request.\nelement = get_element()\n",
        ),
        (
            "tensor_shape.py",
            "# (B, N, num_heads, d)\nvalue = value.view(B, N, num_heads, d)\n",
        ),
        (
            "sphinx_attribute.py",
            "#: A UUID parameter.\nuuid_parameter = UUID\n",
        ),
        (
            "divider.py",
            "# --- object name ---\nobject_name = get_object_name()\n",
        ),
        (
            "scenario_state.py",
            "# The response is logged\nassert response == \"logged\"\n",
        ),
        (
            "additional_information.py",
            "# the default value is kept normalized to the type of the choice\ndefault_value = normalize(choice, choice_type)\n",
        ),
    ];
    for (name, source) in sources {
        fs::write(directory.join(name), place_below_header(source))
            .expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR007",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_no_subsumed_comments_boundaries_synonyms_and_suppression() {
    let directory = create_temp_directory("no-subsumed-comments-boundaries");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nper-file-ignores = { \"ignored.py\" = [\"GR007\"] }\n",
    )
    .expect("test configuration should be written");
    // Columns: file name, source, whether GR007 flags it, whether it is placed below the shared
    // header. The header-floor fixture is the one case that must sit at the top of its file.
    let sources = [
        (
            "first_five_lines.py",
            "\n\n\n\n# Get the element\nelement = get_element()\n",
            false,
            false,
        ),
        (
            "comment_between.py",
            "# Get the element\n\n# helper note\nelement = get_element()\n",
            true,
            true,
        ),
        (
            "trailing.py",
            "element = get_element()  # Get the element\nnext_element = get_element()\n",
            false,
            true,
        ),
        (
            "single_word.py",
            "# Element\nelement = get_element()\n",
            false,
            true,
        ),
        (
            "synonym.py",
            "# Check if user is active\nif user_active:\n    pass\n",
            true,
            true,
        ),
        (
            "suppressed.py",
            "# Get the element  # noqa: GR007\nelement = get_element()\n",
            false,
            true,
        ),
        (
            "ignored.py",
            "# Get the element\nelement = get_element()\n",
            false,
            true,
        ),
        (
            "fstring_scenario.py",
            "# The response text is logged\nassert response.text == f\"Logged {state}\"\n",
            false,
            true,
        ),
        (
            "fstring_comparison_segment.py",
            "# Expected logged\nvalue = f\"\"\"prefix\n{response == expected} logged\"\"\"\n",
            false,
            true,
        ),
        (
            "multiple_hashes.py",
            "## --- object name ---\nobject_name = get_object_name()\n",
            false,
            true,
        ),
        (
            "acronym_digits.py",
            "# HTTP server\nHTTP2Server()\n",
            true,
            true,
        ),
        (
            "fstring_segments.py",
            "# State logged\nassert response == f\"\"\"prefix\n{state} logged\"\"\"\n",
            true,
            true,
        ),
        (
            "interior_blanks.py",
            "# Build cached result\nresult = build(\n\n    value,\n\n    cached_result,\n)\n",
            false,
            true,
        ),
        (
            "physical_line_window.py",
            "# Build cached result\nresult = build(\n    value,\n)\n\ndef test_cached_result():\n    pass\n",
            false,
            true,
        ),
        (
            "doubled_hash_directive.py",
            "# Get the element  ## noqa: GR001\nelement = get_element()\n",
            true,
            true,
        ),
        (
            "leading_blank_gap.py",
            "# Get the element\n\n\n\nelement = get_element()\n",
            true,
            true,
        ),
    ];
    for (name, source, _, places_below_header) in sources {
        let source = if places_below_header {
            place_below_header(source)
        } else {
            source.to_owned()
        };
        fs::write(directory.join(name), source).expect("boundary source should be written");
    }
    let mut expected_filenames: Vec<_> = sources
        .iter()
        .filter(|(_, _, is_flagged, _)| *is_flagged)
        .map(|(name, _, _, _)| *name)
        .collect();
    expected_filenames.sort_unstable();

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--select", "GR007", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), expected_filenames.len());
    for finding in findings.as_array().unwrap() {
        assert_eq!(finding["code"], "GR007");
    }
    let filenames: Vec<_> = findings
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            PathBuf::from(finding["filename"].as_str().unwrap())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(filenames, expected_filenames);

    for selector in ["GR", "ALL"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
            .args([
                "check",
                "--isolated",
                "--select",
                selector,
                "--output-format",
                "json",
                "synonym.py",
            ])
            .current_dir(&directory)
            .output()
            .expect("gruff should run");
        let findings: Value =
            serde_json::from_slice(&output.stdout).expect("output should be JSON");
        assert_eq!(findings.as_array().unwrap().len(), 1);
        assert_eq!(findings[0]["code"], "GR007");
    }

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn flags_no_exception_swallowing_tests_conformance_cases() {
    let directory = create_temp_directory("no-exception-swallowing-tests-findings");
    let mut sources = [
        (
            "test_async.py",
            "async def test_fetch():\n    try:\n        await fetch()\n    except ValueError:\n        pass\n",
            &[(4, 5)][..],
        ),
        (
            "test_bare_except.py",
            "def test_fetch():\n    try:\n        fetch()\n    except:\n        pass\n",
            &[(4, 5)][..],
        ),
        (
            "test_bare_return.py",
            "def test_fetch():\n    try:\n        fetch()\n    except OSError:\n        return\n",
            &[(4, 5)][..],
        ),
        (
            "test_cls_skiptest.py",
            "class FetchTest(unittest.TestCase):\n    @classmethod\n    def test_fetch(cls):\n        try:\n            fetch()\n        except OSError:\n            cls.skipTest(\"x\")\n",
            &[(6, 9)][..],
        ),
        (
            "test_docstring_only.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        \"just a note\"\n",
            &[(4, 5)][..],
        ),
        (
            "test_docstring_then_pass.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        \"\"\"The service is flaky.\"\"\"\n        pass\n",
            &[(4, 5)][..],
        ),
        (
            "test_else_skip_handler.py",
            "def test_fetch():\n    try:\n        result = fetch()\n    except ConnectionError:\n        pytest.skip(\"no net\")\n    else:\n        assert result\n",
            &[(4, 5)][..],
        ),
        (
            "test_ellipsis.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        ...\n",
            &[(4, 5)][..],
        ),
        (
            "test_except_star.py",
            "def test_fetch():\n    try:\n        fetch()\n    except* ValueError:\n        pass\n",
            &[(4, 5)][..],
        ),
        (
            "test_multiline_tuple.py",
            "def test_fetch():\n    try:\n        fetch()\n    except (\n        ValueError,\n        OSError,\n    ):\n        pass\n",
            &[(4, 5)][..],
        ),
        (
            "test_multiple_handlers.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        pass\n    except OSError:\n        pytest.skip(\"no net\")\n",
            &[(4, 5), (6, 5)][..],
        ),
        (
            "test_nested_function.py",
            "def test_fetch():\n    def attempt():\n        try:\n            fetch()\n        except ValueError:\n            pass\n\n    attempt()\n",
            &[(5, 9)][..],
        ),
        (
            "test_nested_loop.py",
            "def test_fetch():\n    for path in paths:\n        try:\n            fetch(path)\n        except ValueError:\n            pass\n",
            &[(5, 9)][..],
        ),
        (
            "test_pass.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        pass\n",
            &[(4, 5)][..],
        ),
        (
            "test_sibling_reraise.py",
            "def test_fetch():\n    try:\n        fetch()\n    except OSError:\n        raise\n    except ValueError:\n        pass\n",
            &[(6, 5)][..],
        ),
        (
            "test_skip.py",
            "def test_fetch():\n    try:\n        fetch()\n    except Exception:\n        pytest.skip(\"flaky\")\n",
            &[(4, 5)][..],
        ),
        (
            "test_try_inside_else.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        pass\n    else:\n        try:\n            check()\n        except KeyError:\n            pass\n",
            &[(9, 9)][..],
        ),
        (
            "test_with_block.py",
            "def test_fetch():\n    with client() as session:\n        try:\n            session.fetch()\n        except ValueError:\n            pass\n",
            &[(5, 9)][..],
        ),
        (
            "unittest_test.py",
            "class FetchTest(unittest.TestCase):\n    def test_fetch(self):\n        try:\n            fetch()\n        except OSError:\n            self.skipTest(\"no net\")\n",
            &[(5, 9)][..],
        ),
    ];
    sources.sort_unstable_by_key(|(name, _, _)| *name);
    for (name, source, _) in &sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }
    let expected: Vec<(&str, (usize, usize))> = sources
        .iter()
        .flat_map(|(name, _, locations)| locations.iter().map(|location| (*name, *location)))
        .collect();

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR008",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), expected.len());
    for (finding, (name, (row, column))) in findings.as_array().unwrap().iter().zip(expected) {
        assert!(finding["filename"].as_str().unwrap().ends_with(name));
        assert_eq!(finding["code"], "GR008");
        assert_eq!(finding["name"], "no-exception-swallowing-tests");
        assert_eq!(
            finding["message"],
            "Test swallows the exception, so it cannot fail; let it propagate, or use pytest.raises or a skip condition for the expected case"
        );
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
        assert_eq!(finding["noqa_row"], row);
        // The range must span to the closing parenthesis, not stop on the `except` line.
        if name == "test_multiline_tuple.py" {
            assert_eq!(finding["end_location"]["row"], 7);
            assert_eq!(finding["end_location"]["column"], 6);
        }
    }

    for selector in ["GR", "ALL"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
            .args([
                "check",
                "--isolated",
                "--select",
                selector,
                "--output-format",
                "json",
                "test_pass.py",
            ])
            .current_dir(&directory)
            .output()
            .expect("gruff should run");
        let findings: Value =
            serde_json::from_slice(&output.stdout).expect("output should be JSON");
        assert_eq!(findings.as_array().unwrap().len(), 1);
        assert_eq!(findings[0]["code"], "GR008");
    }

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_no_exception_swallowing_tests_conformance_cases() {
    let directory = create_temp_directory("no-exception-swallowing-tests-allowed");
    let sources = [
        (
            "production.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        pass\n",
        ),
        (
            "test_asserts.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError as error:\n        assert \"missing\" in str(error)\n",
        ),
        (
            "test_class_in_helper.py",
            "def make_case():\n    class TestFetch:\n        def test_fetch(self):\n            try:\n                fetch()\n            except ValueError:\n                pass\n\n    return TestFetch\n",
        ),
        (
            "test_break.py",
            "def test_fetch():\n    for path in paths:\n        try:\n            fetch(path)\n        except ValueError:\n            break\n",
        ),
        (
            "test_continue.py",
            "def test_fetch():\n    for path in paths:\n        try:\n            fetch(path)\n        except ValueError:\n            continue\n",
        ),
        (
            "test_else_clause.py",
            "def test_fetch_raises_value_error():\n    try:\n        fetch()\n    except ValueError:\n        pass\n    else:\n        raise AssertionError(\"expected ValueError\")\n",
        ),
        (
            "test_fixture_hooks.py",
            "class FetchTest(unittest.TestCase):\n    def setUp(self):\n        try:\n            connect()\n        except OSError:\n            self.skipTest(\"no net\")\n",
        ),
        (
            "test_finally.py",
            "def test_fetch():\n    try:\n        fetch()\n    finally:\n        close()\n",
        ),
        (
            "test_helper.py",
            "def build_client():\n    try:\n        fetch()\n    except ValueError:\n        pass\n",
        ),
        (
            "test_logs.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        logger.warning(\"fetch failed\")\n        pass\n",
        ),
        (
            "test_raises_context.py",
            "def test_fetch():\n    with pytest.raises(ValueError):\n        fetch()\n",
        ),
        (
            "test_return_none.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        return None\n",
        ),
        (
            "test_reraise.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        raise\n\n\ndef test_chain():\n    try:\n        fetch()\n    except ValueError as error:\n        raise AssertionError from error\n",
        ),
        (
            "test_module_level.py",
            "try:\n    import fast\nexcept ImportError:\n    pass\n\n\ndef test_fetch():\n    assert fetch()\n",
        ),
        (
            "test_skip_outside_handler.py",
            "def test_fetch():\n    if not has_network:\n        pytest.skip(\"no network\")\n    assert fetch()\n",
        ),
        (
            "test_skiptest_raise.py",
            "def test_fetch():\n    try:\n        fetch()\n    except OSError:\n        raise unittest.SkipTest(\"x\")\n",
        ),
        (
            "test_suppress.py",
            "def test_fetch():\n    with contextlib.suppress(ValueError):\n        fetch()\n    assert state()\n",
        ),
        // A bare name is not resolved to an import, so only the `pytest.skip` spelling counts.
        (
            "test_unqualified_skip.py",
            "from pytest import skip\n\n\ndef test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        skip(\"flaky\")\n",
        ),
        (
            "test_xfail.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        pytest.xfail(\"known\")\n",
        ),
        ("test_smoke_call.py", "def test_fetch():\n    fetch()\n"),
        (
            "test_suppressed.py",
            "def test_fetch():\n    try:\n        fetch()\n    except ValueError:  # noqa: GR008\n        pass\n",
        ),
    ];
    for (name, source) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR008",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn flags_no_guarded_tails_conformance_cases() {
    let directory = create_temp_directory("no-guarded-tails-findings");
    let mut sources = [
        (
            "async_function.py",
            "async def refresh(session):\n    token = await session.token()\n    if token.is_expired:\n        await session.revoke(token)\n        if session.can_renew:\n            await session.renew()\n",
            (3, 5),
        ),
        (
            "docstring_prefix.py",
            "def cleanup(session):\n    \"\"\"Release the session's resources.\"\"\"\n    if session.is_open:\n        session.flush()\n        if session.has_pending():\n            session.wait()\n",
            (3, 5),
        ),
        // Ten physical lines of straight-line work, with no nested conditional.
        (
            "function_long_suite.py",
            "def render(invoice):\n    header = build_header(invoice)\n    if invoice.line_items:\n        rows = []\n        for item in invoice.line_items:\n            rows.append(format_row(item))\n        total = compute_total(invoice)\n        footer = format_total(total)\n        body = join_rows(rows)\n        document = header + body + footer\n        log_render(invoice, document)\n        store(invoice, document)\n        return document\n",
            (3, 5),
        ),
        // Four statements that reach ten physical lines only through the blank and comment lines
        // between them.
        (
            "interior_blank_lines.py",
            "def publish(article):\n    draft = load(article)\n    if draft.is_ready:\n        validate(draft)\n\n        # The queue rejects a second copy, so the guard runs here.\n\n        register(draft)\n\n        notify(draft)\n\n\n        publish_now(draft)\n",
            (3, 5),
        ),
        (
            "loop_nested_if.py",
            "def sync(records):\n    for record in records:\n        load(record)\n        if record.is_dirty:\n            record.normalize()\n            if record.is_valid:\n                record.save()\n",
            (4, 9),
        ),
        (
            "nested_function.py",
            "def build():\n    def apply(record):\n        prepare(record)\n        if record.is_active:\n            record.touch()\n            if record.is_stale:\n                record.refresh()\n\n    return apply\n",
            (4, 9),
        ),
        // Only the outer of two directly nested trailing ifs is a direct child of the body, so the
        // inner one waits for the run after the outer is inverted.
        (
            "nested_trailing_ifs.py",
            "def apply():\n    job = fetch_job()\n    if job.is_ready:\n        prepare(job)\n        if job.is_urgent:\n            escalate(job)\n",
            (3, 5),
        ),
        (
            "while_loop.py",
            "def drain(queue):\n    while queue:\n        item = queue.pop()\n        if item.is_valid:\n            record(item)\n            if item.is_last:\n                finish(item)\n",
            (4, 9),
        ),
    ];
    sources.sort_unstable_by_key(|(name, _, _)| *name);
    for (name, source, _) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR009",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), sources.len());
    for (finding, (name, _, (row, column))) in findings.as_array().unwrap().iter().zip(sources) {
        assert!(finding["filename"].as_str().unwrap().ends_with(name));
        assert_eq!(finding["code"], "GR009");
        assert_eq!(finding["name"], "no-guarded-tails");
        assert_eq!(
            finding["message"],
            "Trailing `if` nests the rest of the body in its condition; invert it into an early `return` or `continue` guard"
        );
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
        assert_eq!(finding["noqa_row"], row);
    }

    for selector in ["GR", "ALL"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
            .args([
                "check",
                "--isolated",
                "--select",
                selector,
                "--output-format",
                "json",
                "nested_trailing_ifs.py",
            ])
            .current_dir(&directory)
            .output()
            .expect("gruff should run");
        let findings: Value =
            serde_json::from_slice(&output.stdout).expect("output should be JSON");
        assert_eq!(findings.as_array().unwrap().len(), 1);
        assert_eq!(findings[0]["code"], "GR009");
    }

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_no_guarded_tails_conformance_cases() {
    let directory = create_temp_directory("no-guarded-tails-allowed");
    let sources = [
        (
            "class_body.py",
            "class Service:\n    name = \"service\"\n    if TYPE_CHECKING:\n        client = None\n        if DEBUG:\n            trace = None\n",
        ),
        (
            "except_body.py",
            "def apply(job):\n    try:\n        prepare(job)\n    except OSError:\n        log(job)\n        if job.is_ready:\n            escalate(job)\n            if job.is_urgent:\n                notify(job)\n",
        ),
        (
            "finally_body.py",
            "def apply(job):\n    try:\n        prepare(job)\n    finally:\n        log(job)\n        if job.is_ready:\n            escalate(job)\n            if job.is_urgent:\n                notify(job)\n",
        ),
        (
            "loop_else.py",
            "def apply(jobs):\n    for job in jobs:\n        prepare(job)\n    else:\n        log(jobs)\n        if jobs:\n            escalate(jobs)\n            if len(jobs) > 1:\n                notify(jobs)\n",
        ),
        (
            "match_only.py",
            "def render(invoice):\n    header = build_header(invoice)\n    if invoice.line_items:\n        match invoice.status:\n            case \"paid\":\n                label = \"paid\"\n            case _:\n                label = \"due\"\n        return header + label\n",
        ),
        (
            "module_level.py",
            "config = load_config()\nif config.is_valid:\n    apply(config)\n    if config.is_strict:\n        enforce(config)\n",
        ),
        (
            "not_last.py",
            "def apply(job):\n    if job.is_ready:\n        prepare(job)\n        if job.is_urgent:\n            escalate(job)\n    finish(job)\n",
        ),
        // Nine physical lines, one short of the gate, with no nested conditional.
        (
            "sub_gate.py",
            "def render(invoice):\n    header = build_header(invoice)\n    if invoice.line_items:\n        rows = []\n        for item in invoice.line_items:\n            rows.append(format_row(item))\n        total = compute_total(invoice)\n        footer = format_total(total)\n        body = join_rows(rows)\n        document = header + body + footer\n        log_render(invoice, document)\n        return document\n",
        ),
        (
            "suppressed.py",
            "def apply(job):\n    if job.is_ready:  # noqa: GR009 -- kept parallel with the sibling branch\n        prepare(job)\n        if job.is_urgent:\n            escalate(job)\n",
        ),
        (
            "ternary_only.py",
            "def render(invoice):\n    header = build_header(invoice)\n    if invoice.line_items:\n        total = compute_total(invoice)\n        label = \"paid\" if invoice.is_paid else \"due\"\n        return header + label + str(total)\n",
        ),
        (
            "try_body.py",
            "def apply(job):\n    try:\n        prepare(job)\n        if job.is_ready:\n            escalate(job)\n            if job.is_urgent:\n                notify(job)\n    except OSError:\n        log(job)\n",
        ),
        (
            "with_body.py",
            "def apply(job):\n    with lock(job):\n        prepare(job)\n        if job.is_ready:\n            escalate(job)\n            if job.is_urgent:\n                notify(job)\n",
        ),
        (
            "with_elif.py",
            "def apply(job):\n    if job.is_ready:\n        prepare(job)\n        if job.is_urgent:\n            escalate(job)\n    elif job.is_stale:\n        drop(job)\n",
        ),
        (
            "with_else.py",
            "def apply(job):\n    if job.is_ready:\n        prepare(job)\n        if job.is_urgent:\n            escalate(job)\n    else:\n        defer(job)\n",
        ),
    ];
    for (name, source) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR009",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn flags_positive_branch_conditions_conformance_cases() {
    let directory = create_temp_directory("positive-branch-conditions-findings");
    let mut sources = [
        (
            "not_equal.py",
            "if a != b:\n    diverge(a, b)\nelse:\n    converge(a, b)\n",
            (1, 1),
        ),
        (
            "not_in.py",
            "if a not in b:\n    add(a, b)\nelse:\n    skip(a, b)\n",
            (1, 1),
        ),
        (
            "is_not_none.py",
            "if record is not None:\n    apply(record)\nelse:\n    log_missing()\n",
            (1, 1),
        ),
        (
            "unary_not.py",
            "if not ready:\n    wait()\nelse:\n    start()\n",
            (1, 1),
        ),
        // The outermost node is the `not`, so the inner comparison does not matter.
        (
            "unary_not_over_comparison.py",
            "if not (a == b):\n    diverge(a, b)\nelse:\n    converge(a, b)\n",
            (1, 1),
        ),
        (
            "nested_in_function.py",
            "def apply(job):\n    for step in job.steps:\n        if step not in DONE:\n            run(step)\n        else:\n            skip(step)\n",
            (3, 9),
        ),
    ];
    sources.sort_unstable_by_key(|(name, _, _)| *name);
    for (name, source, _) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR010",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), sources.len());
    for (finding, (name, _, (row, column))) in findings.as_array().unwrap().iter().zip(sources) {
        assert!(finding["filename"].as_str().unwrap().ends_with(name));
        assert_eq!(finding["code"], "GR010");
        assert_eq!(finding["name"], "positive-branch-conditions");
        assert_eq!(
            finding["message"],
            "Negated `if` condition with an `else`; test the positive form and swap the branches"
        );
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
        assert_eq!(finding["noqa_row"], row);
    }

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn flags_each_nested_positive_branch_condition() {
    let directory = create_temp_directory("positive-branch-conditions-nested");
    fs::write(
        directory.join("nested.py"),
        "if not ready:\n    if value is not None:\n        apply(value)\n    else:\n        clear()\nelse:\n    start()\n",
    )
    .expect("conformance source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR010",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 2);
    assert_eq!(findings[0]["location"]["row"], 1);
    assert_eq!(findings[1]["location"]["row"], 2);

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_positive_branch_conditions_conformance_cases() {
    let directory = create_temp_directory("positive-branch-conditions-allowed");
    let sources = [
        (
            "chained_comparison.py",
            "if a is not b is not c:\n    diverge(a, b)\nelse:\n    converge(a, b)\n",
        ),
        (
            "elif_without_else.py",
            "if not ready:\n    wait()\nelif stalled:\n    reset()\n",
        ),
        (
            "membership.py",
            "if a in b:\n    skip(a, b)\nelse:\n    add(a, b)\n",
        ),
        (
            "negation_under_and.py",
            "if x and not y:\n    apply(x)\nelse:\n    skip(x)\n",
        ),
        (
            "negation_under_or.py",
            "if x or not y:\n    apply(x)\nelse:\n    skip(x)\n",
        ),
        ("no_else.py", "if not ready:\n    wait()\n"),
        (
            "positive_test.py",
            "if record is None:\n    log_missing()\nelse:\n    apply(record)\n",
        ),
        (
            "suppressed.py",
            "if version != EXPECTED:  # noqa: GR010 -- Version.__ne__ compares ranges\n    reject(version)\nelse:\n    accept(version)\n",
        ),
        ("ternary.py", "z = a if not x else b\n"),
        (
            "with_elif.py",
            "if not ready:\n    wait()\nelif stalled:\n    reset()\nelse:\n    start()\n",
        ),
    ];
    for (name, source) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR010",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn flags_no_single_consumer_module_bindings_conformance_cases() {
    let directory = create_temp_directory("no-single-consumer-module-bindings-findings");
    let mut sources = [
        (
            "annotated.py",
            "from typing import Final\n\n_LIMIT: Final = 4096\n\n\ndef validate(width, /):\n    return width <= _LIMIT\n",
            ("_LIMIT", "validate", 3, 1),
        ),
        (
            "async_lambda.py",
            "_SCALE = 2\n\n\nasync def scale(values, /):\n    return map(lambda value: value * _SCALE, values)\n",
            ("_SCALE", "scale", 1, 1),
        ),
        // Qt's `.exec()` is attribute access, not the builtin that hides the namespace.
        (
            "attribute_exec.py",
            "_TITLE = \"Settings\"\n\n\nclass Window:\n    @classmethod\n    def open(cls):\n        dialog = cls.build(_TITLE)\n        dialog.exec()\n",
            ("_TITLE", "Window.open", 1, 1),
        ),
        // `_A` is read at module level by `_B`'s value, so only `_B` is flagged on this run.
        (
            "cascade.py",
            "_A = 1\n_B = _A + 1\n\n\ndef read():\n    return _B\n",
            ("_B", "read", 2, 1),
        ),
        (
            "comprehension.py",
            "_MAX = 255\n\n\ndef is_rgb(value, /):\n    return all(0 <= channel <= _MAX for channel in value)\n",
            ("_MAX", "is_rgb", 1, 1),
        ),
        (
            "lowercase.py",
            "_suffix = \".png\"\n\n\ndef icon_path(name, /):\n    return name + _suffix\n",
            ("_suffix", "icon_path", 1, 1),
        ),
        (
            "method.py",
            "_FORMATS = {\"RGB\": 1, \"RGBA\": 2}\n\n\nclass Dialog:\n    def apply(self):\n        return _FORMATS[self.mode]\n",
            ("_FORMATS", "Dialog.apply", 1, 1),
        ),
        // Two reads from the same consumer are still one consumer.
        // A lookup reads the container; only a store into it would keep the binding.
        (
            "lookup.py",
            "_SCHEMES = {\"light\": 1, \"dark\": 2}\n\n\ndef scheme(theme, /):\n    return _SCHEMES.get(theme, 0)\n",
            ("_SCHEMES", "scheme", 1, 1),
        ),
        (
            "multiple_reads.py",
            "_LABEL = \"Zoom\"\n\n\ndef build(widget, /):\n    widget.setToolTip(_LABEL)\n    widget.setStatusTip(_LABEL)\n",
            ("_LABEL", "build", 1, 1),
        ),
        (
            "nested_function.py",
            "_SEED = 9\n\n\ndef outer():\n    def inner():\n        return _SEED\n\n    return inner\n",
            ("_SEED", "outer", 1, 1),
        ),
        // The read sits in a static method, so the consumer is spelled with its class.
        (
            "static_method.py",
            "_ORIGIN = (0, 0)\n\n\nclass Canvas:\n    @staticmethod\n    def origin():\n        return _ORIGIN\n",
            ("_ORIGIN", "Canvas.origin", 1, 1),
        ),
    ];
    sources.sort_unstable_by_key(|(name, _, _)| *name);
    for (name, source, _) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR011",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), sources.len());
    for (finding, (name, _, (binding, consumer, row, column))) in
        findings.as_array().unwrap().iter().zip(sources)
    {
        assert!(finding["filename"].as_str().unwrap().ends_with(name));
        assert_eq!(finding["code"], "GR011");
        assert_eq!(finding["name"], "no-single-consumer-module-bindings");
        assert_eq!(
            finding["message"],
            format!(
                "Non-public module binding `{binding}` is used only by `{consumer}`; move it into that definition"
            )
        );
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
        assert_eq!(finding["noqa_row"], row);
    }

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_no_single_consumer_module_bindings_conformance_cases() {
    let directory = create_temp_directory("no-single-consumer-module-bindings-allowed");
    let sources = [
        (
            "annotation_read.py",
            "_Mode = int\n\n\ndef parse(value: _Mode, /):\n    return value\n",
        ),
        (
            "called_value.py",
            "_CONFIG = load_config()\n\n\ndef default(key, /):\n    return _CONFIG[key]\n",
        ),
        (
            "class_body_read.py",
            "_HEIGHT = 6\n\n\nclass Widget:\n    height = _HEIGHT\n\n    def build(self):\n        return _HEIGHT\n",
        ),
        (
            "conditional_definition.py",
            "_FALLBACK = 1\n\nif PY_OLD:\n    def load():\n        return _FALLBACK\n",
        ),
        (
            "decorator_read.py",
            "_CASES = (1, 2)\n\n\n@parametrize(_CASES)\ndef test_case(case, /):\n    return case in _CASES\n",
        ),
        (
            "default_read.py",
            "_TEXT = \"label\"\n\n\ndef render(text=_TEXT, /):\n    return text + _TEXT\n",
        ),
        (
            "attribute_store.py",
            "_STATE = Namespace\n\n\ndef mark():\n    _STATE.ready = True\n",
        ),
        // The stored-into target is nested; the base of the chain is what changes.
        (
            "chained_store.py",
            "_CACHE = {\"a\": {}}\n_STATE = Namespace\n_ROWS = [Row]\n_TABLE = Namespace\n\n\ndef mark(key, /):\n    _CACHE[\"a\"][key] = 1\n    _STATE.inner.ready = True\n    _ROWS[0].ready = True\n    del _TABLE.rows[key]\n",
        ),
        (
            "dunder_all.py",
            "_LEVELS = (\"debug\", \"info\")\n__all__ = [\"_LEVELS\"]\n\n\ndef main():\n    return _LEVELS\n",
        ),
        (
            "dunder_all_extended.py",
            "_LEVELS = (\"debug\", \"info\")\n__all__ = ()\n__all__ += (\"_LEVELS\",)\n\n\ndef main():\n    return _LEVELS\n",
        ),
        (
            "empty_accumulator.py",
            "_SEEN = []\n\n\ndef add(item, /):\n    _SEEN.append(item)\n    return len(_SEEN)\n",
        ),
        (
            "mangled.py",
            "__SIZE = 5\n\n\nclass Canvas:\n    def size(self):\n        return __SIZE\n",
        ),
        (
            "dynamic_namespace.py",
            "_LIMIT = 1\n\n\ndef read(name, /):\n    return _LIMIT + globals()[name]\n",
        ),
        (
            "global_rebind.py",
            "_SESSION = None\n\n\ndef session():\n    global _SESSION\n    _SESSION = 1\n    return _SESSION\n",
        ),
        (
            "module_read.py",
            "_OFFSET = 0.22\n_WIDTH = _OFFSET * 2\n\n\ndef arrow():\n    return _OFFSET\n",
        ),
        (
            "nested_class_method.py",
            "_SIZE = 3\n\n\nclass Outer:\n    class Inner:\n        def size(self):\n            return _SIZE\n",
        ),
        (
            "public_name.py",
            "LIMIT = 4096\n\n\ndef validate(width, /):\n    return width <= LIMIT\n",
        ),
        (
            "rebound.py",
            "_COUNT = 1\n_COUNT = 2\n\n\ndef count():\n    return _COUNT\n",
        ),
        // Every other binding form counts as a second binding site.
        (
            "rebound_elsewhere.py",
            "_A = 1\n_B = 2\n_C = 3\n_D = 4\n_E = 5\n_F = 6\n_G = 7\n\n\ndef read(_A, /):\n    import os as _B\n    for _C in ():\n        pass\n    with open(\"f\") as _D:\n        pass\n    try:\n        pass\n    except OSError as _E:\n        pass\n    match _F:\n        case _F:\n            pass\n    if (_G := 1):\n        pass\n    return _A, _B, _C, _D, _E, _F, _G\n",
        ),
        (
            "shadowed.py",
            "_VALUE = 8\n\n\ndef read():\n    return _VALUE\n\n\ndef write():\n    _VALUE = 1\n    return _VALUE\n",
        ),
        // The only read is inside a string, which is a constant, so the binding has no consumer.
        (
            "string_annotation.py",
            "_Alias = int\n\n\ndef parse(value, /):\n    parsed: \"_Alias\" = value\n    return parsed\n",
        ),
        (
            "subscript_store.py",
            "_CACHE = {\"seed\": 0}\n\n\ndef get(key, /):\n    if key not in _CACHE:\n        _CACHE[key] = len(_CACHE)\n    return _CACHE[key]\n",
        ),
        (
            "suppressed.py",
            "_LEVELS = (\"debug\", \"info\")  # noqa: GR011 -- the test module parametrizes over it\n\n\ndef main():\n    return _LEVELS\n",
        ),
        (
            "suppressed_multiline.py",
            "_LEVELS = (  # noqa: GR011 -- the test module parametrizes over it\n    \"debug\",\n    \"info\",\n)\n\n\ndef main():\n    return _LEVELS\n",
        ),
        (
            "two_consumers.py",
            "_TWO = 2\n\n\ndef first():\n    return _TWO\n\n\ndef second():\n    return _TWO\n",
        ),
        (
            "unpacked.py",
            "_A, _B = 1, 2\n\n\ndef read():\n    return _A + _B\n",
        ),
        ("unused.py", "_DEAD = 1\n\n\ndef read():\n    return 2\n"),
    ];
    for (name, source) in sources {
        fs::write(directory.join(name), source).expect("conformance source should be written");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR011",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn splits_input_convention_rules_under_prefix_selection() {
    let directory = create_temp_directory("split-input-conventions");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff]\noutput-format = \"json\"\n\n[tool.gruff.lint]\nselect = [\"GR\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(
        directory.join("definitions.py"),
        "def public(path):\n    ...\n\ndef _non_public(path=None):  # noqa: GR001\n    ...\n\ndef public_suppressed(path):  # noqa: GR005\n    ...\n\ndef _non_public_suppressed(path):  # noqa: GR002\n    ...\n",
    )
    .expect("test source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 3);
    assert_eq!(findings[0]["code"], "GR005");
    assert_eq!(findings[0]["name"], "explicit-public-input-conventions");
    assert_eq!(findings[1]["code"], "GR002");
    assert_eq!(findings[2]["code"], "GR001");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--select", "ALL", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 3);
    assert_eq!(findings[0]["code"], "GR005");
    assert_eq!(findings[1]["code"], "GR002");
    assert_eq!(findings[2]["code"], "GR001");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--ignore", "GR001", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 2);
    assert_eq!(findings[0]["code"], "GR005");
    assert_eq!(findings[1]["code"], "GR002");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--select", "GR005", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["code"], "GR005");

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_package_dunder_all_findings_locations_output_and_exit_status() {
    let directory = create_temp_directory("package-dunder-all-findings");
    let sources = [
        (
            "aggregator/__init__.py",
            "from .api import Public as Public\nother = 1\n",
        ),
        (
            "definitions/__init__.py",
            "def public():\n    ...\nclass Other:\n    ...\n",
        ),
        ("class-only/__init__.py", "class Public:\n    ...\n"),
        (
            "decorator-binding/__init__.py",
            "@(public_decorator := identity)\ndef _private():\n    ...\n",
        ),
        (
            "default-binding/__init__.py",
            "def _private(value=(public_default := 1)):\n    ...\n",
        ),
        (
            "lambda-default-binding/__init__.py",
            "_callable = lambda value=(public_default := 1): value\n",
        ),
        (
            "base-binding/__init__.py",
            "class _Private((public_base := object)):\n    ...\n",
        ),
        (
            "deleted-all/__init__.py",
            "public = 1\n__all__ = []\ndel __all__\n",
        ),
        ("destructuring/__init__.py", "left, right = _values\n"),
        (
            "for-target/__init__.py",
            "for loop_public in _values:\n    break\n",
        ),
        (
            "match-target/__init__.py",
            "match _value:\n    case match_public:\n        pass\n",
        ),
        (
            "match-failed-guard/__init__.py",
            "match _value:\n    case guarded_public if False:\n        pass\n",
        ),
        (
            "path/__init__.py",
            "if condition:\n    public = 1\n    __all__ = []\nelse:\n    alternate = 1\n",
        ),
        ("stub/__init__.pyi", "declared: int\n"),
        ("type-alias/__init__.py", "type Public = int\n"),
        (
            "target-expression/__init__.py",
            "_holder[(public := 1)] = 0\n",
        ),
        (
            "unrelated-type-checking/__init__.py",
            "from .config import flags as _flags\nif _flags.TYPE_CHECKING:\n    public = 1\n",
        ),
        (
            "relative-type-checking/__init__.py",
            "from .typing import TYPE_CHECKING as _TYPE_CHECKING\nif _TYPE_CHECKING:\n    public = 1\n",
        ),
        (
            "bare-handler/__init__.py",
            "try:\n    raise UnknownError\nexcept:\n    public = 1\n",
        ),
        (
            "handler-type-binding/__init__.py",
            "try:\n    raise RuntimeError\nexcept (public := RuntimeError):\n    pass\n",
        ),
        (
            "try-handler/__init__.py",
            "try:\n    raise Error\nexcept Error as error:\n    handler_public = 1\n",
        ),
        (
            "while-target/__init__.py",
            "while condition:\n    while_public = 1\n    break\n",
        ),
        (
            "with-target/__init__.py",
            "with _manager() as context_public:\n    pass\n",
        ),
        (
            "finally-binding/__init__.py",
            "try:\n    pass\nfinally:\n    finalized = 1\n",
        ),
    ];
    for (relative_path, source) in sources {
        let path = directory.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).expect("package should be created");
        fs::write(path, source).expect("package initializer should be written");
    }
    fs::write(directory.join("ordinary.py"), "public = 1\n")
        .expect("ordinary module should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR003",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), sources.len());
    for finding in findings.as_array().unwrap() {
        assert_eq!(finding["code"], "GR003");
        assert_eq!(finding["name"], "package-dunder-all");
        assert_eq!(
            finding["message"],
            "Package initializer with public bindings must define __all__ on every import path"
        );
        assert_eq!(finding["severity"], "error");
    }
    let aggregator = findings
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| {
            finding["filename"]
                .as_str()
                .unwrap()
                .ends_with("aggregator/__init__.py")
        })
        .unwrap();
    assert_eq!(aggregator["location"]["row"], 1);
    assert_eq!(aggregator["location"]["column"], 28);

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_package_dunder_all_safe_completion_paths() {
    let directory = create_temp_directory("package-dunder-all-allowed");
    let sources = [
        ("empty/__init__.py", ""),
        ("assert-false/__init__.py", "assert False\npublic = 1\n"),
        (
            "future-only/__init__.py",
            "from __future__ import annotations\n",
        ),
        (
            "private/__init__.py",
            "__version__ = \"1\"\n_private = 1\ndeclared: int\n",
        ),
        (
            "direct/__init__.py",
            "public = 1\n__all__: list[str] = []\n",
        ),
        (
            "imported/__init__.py",
            "from .exports import __all__ as __all__\nfrom .api import public\n",
        ),
        (
            "function-manifest/__init__.py",
            "public = 1\ndef __all__():\n    ...\n",
        ),
        (
            "destructured-manifest/__init__.py",
            "public = 1\n__all__, _metadata = [], None\n",
        ),
        ("deleted/__init__.py", "temporary = 1\ndel temporary\n"),
        (
            "conditional/__init__.py",
            "if condition:\n    public = 1\n    __all__ = []\n",
        ),
        (
            "branches/__init__.py",
            "if condition:\n    public = 1\n    __all__ = []\nelse:\n    alternate = 1\n    __all__ = []\n",
        ),
        (
            "correlated-identity/__init__.py",
            "_flag = get_value()\nif _flag is True:\n    public = 1\nif _flag is not True:\n    pass\nelse:\n    __all__ = []\n",
        ),
        (
            "static/__init__.py",
            "import typing as _typing\nfrom typing_extensions import TYPE_CHECKING as _TYPE_CHECKING\nif TYPE_CHECKING:\n    typed = 1\nif _typing.TYPE_CHECKING:\n    qualified = 1\nif _TYPE_CHECKING:\n    aliased = 1\nif False:\n    disabled = 1\n",
        ),
        (
            "exception/__init__.py",
            "try:\n    raise Error\nexcept Error as error:\n    pass\n",
        ),
        (
            "incompatible-handler/__init__.py",
            "try:\n    raise ValueError\nexcept TypeError:\n    public = 1\n",
        ),
        (
            "qualified-incompatible-handler/__init__.py",
            "class _First:\n    class Error(Exception):\n        pass\nclass _Second:\n    class Error(Exception):\n        pass\ntry:\n    raise _First.Error\nexcept _Second.Error:\n    public = 1\n",
        ),
        (
            "call-qualified-handler/__init__.py",
            "class _Other:\n    Error = TypeError\nclass _Errors:\n    class Error(Exception):\n        pass\n    def __new__(cls):\n        return _Other()\ntry:\n    raise _Errors.Error\nexcept _Errors().Error:\n    public = 1\n",
        ),
        (
            "correlated/__init__.py",
            "if condition:\n    __all__ = []\nif condition:\n    public = 1\n",
        ),
        (
            "unreachable-match/__init__.py",
            "match _value:\n    case _ if True:\n        __all__ = []\n    case _ if True:\n        public = 1\n",
        ),
        ("invalid-augmented/__init__.py", "public += (public := 1)\n"),
        (
            "literal-true/__init__.py",
            "if True:\n    __all__ = []\nelse:\n    pass\npublic = 1\n",
        ),
        (
            "controls/__init__.py",
            "for loop_public in _values:\n    __all__ = []\nwith _manager() as context_public:\n    __all__ = []\nmatch _value:\n    case match_public:\n        __all__ = []\nwhile True:\n    while_public = 1\n    __all__ = []\n    break\n",
        ),
        (
            "nested/__init__.py",
            "def _factory():\n    public = 1\nclass _Holder:\n    public = 1\n",
        ),
    ];
    for (relative_path, source) in sources {
        let path = directory.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).expect("package should be created");
        fs::write(path, source).expect("package initializer should be written");
    }
    fs::write(directory.join("ordinary.py"), "public = 1\n")
        .expect("ordinary module should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR003",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn supports_package_dunder_all_selection_and_suppression() {
    let directory = create_temp_directory("package-dunder-all-suppression");
    for name in ["inline", "ignored"] {
        fs::create_dir(directory.join(name)).expect("package should be created");
    }
    fs::write(
        directory.join("inline/__init__.py"),
        "public = 1  # noqa: GR003 -- generated dynamically\n",
    )
    .expect("suppressed initializer should be written");
    fs::write(directory.join("ignored/__init__.py"), "PUBLIC = 1\n")
        .expect("ignored initializer should be written");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR003\", \"GR004\"]\nper-file-ignores = { \"ignored/__init__.py\" = [\"GR003\"] }\n",
    )
    .expect("configuration should be written");

    let configured = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    assert_eq!(configured.status.code(), Some(1));
    let findings: Value =
        serde_json::from_slice(&configured.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["code"], "GR004");

    let disabled = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--isolated", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    assert!(disabled.status.success());
    assert_eq!(disabled.stdout, b"[]\n");
    assert!(String::from_utf8_lossy(&disabled.stderr).contains("No rules are enabled"));

    let selected = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR003",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    assert_eq!(selected.status.code(), Some(1));
    let findings: Value = serde_json::from_slice(&selected.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["code"], "GR003");
    assert!(
        findings[0]["filename"]
            .as_str()
            .unwrap()
            .ends_with("ignored/__init__.py")
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn bounds_package_dunder_all_branch_analysis_without_speculation() {
    let directory = create_temp_directory("package-dunder-all-bounded");
    let package = directory.join("package");
    fs::create_dir(&package).expect("package should be created");
    let mut source = String::new();
    for index in 0..200 {
        source.push_str(&format!(
            "if condition_{index}:\n    _private_{index} = 1\n"
        ));
    }
    source.push_str("if final_condition:\n    __all__ = []\nelse:\n    public = 1\n");
    fs::write(package.join("__init__.py"), source)
        .expect("branch-heavy initializer should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR003",
            "--output-format",
            "json",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_final_constants_findings_locations_formats_and_exit_status() {
    let directory = create_temp_directory("final-constants-findings");
    let path = directory.join("findings.py");
    fs::write(
        &path,
        r#"MODULE = 1
class Settings:
    CLASS_VALUE: int = 2
    if True:
        NESTED_CLASS = 3
def build():
    FUNCTION_VALUE = 4
    if True:
        variable: Final = 5
_PRIVATE = 6
DECLARATION: int
qualified: typing.Final[int] = 7
OTHER_CODE = 1  # noqa: GR001 -- GR004 is fine
MULTILINE = (
    1,
)  # noqa: GR004 -- external spelling
multiline: Final = (
    2,
)  # noqa: GR004 -- framework state
"#,
    )
    .expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR004",
            "--output-format",
            "json",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    let expected = [
        (1, 1, "Constant MODULE must be annotated Final"),
        (3, 5, "Constant CLASS_VALUE must be annotated Final"),
        (5, 9, "Constant NESTED_CLASS must be annotated Final"),
        (7, 5, "Constant FUNCTION_VALUE must be annotated Final"),
        (
            9,
            9,
            "Final binding variable must be named in UPPER_SNAKE_CASE",
        ),
        (10, 1, "Constant _PRIVATE must be annotated Final"),
        (11, 1, "Constant DECLARATION must be annotated Final"),
        (
            12,
            1,
            "Final binding qualified must be named in UPPER_SNAKE_CASE",
        ),
        (13, 1, "Constant OTHER_CODE must be annotated Final"),
        (14, 1, "Constant MULTILINE must be annotated Final"),
        (
            17,
            1,
            "Final binding multiline must be named in UPPER_SNAKE_CASE",
        ),
    ];
    assert_eq!(findings.as_array().unwrap().len(), expected.len());
    for (finding, (row, column, message)) in findings.as_array().unwrap().iter().zip(expected) {
        assert_eq!(finding["code"], "GR004");
        assert_eq!(finding["name"], "final-constants");
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
        assert_eq!(finding["noqa_row"], row);
        assert_eq!(finding["message"], message);
        assert_eq!(finding["severity"], "error");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR004",
            "--output-format",
            "github",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line=1,col=1,endLine=1,endColumn=7"));
    assert!(stdout.contains("GR004 Constant MODULE must be annotated Final"));
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn allows_final_constants_canonical_and_out_of_scope_bindings() {
    let directory = create_temp_directory("final-constants-allowed");
    fs::write(
        directory.join("allowed.py"),
        r#"from typing import Final, TypeAlias
import enum
import typing
PUBLIC: Final = 1
_PRIVATE: typing.Final[int] = 2
__PRIVATE_2: Final[str] = "value"
ALIAS: TypeAlias = int
QUALIFIED_ALIAS: typing.TypeAlias = str
type RESPONSE = bytes
if True:
    CONTROL_FLOW: Final = 3
class Settings:
    CLASS_VALUE: Final = 4
    if True:
        NESTED_CLASS: Final = 5
def build():
    FUNCTION_VALUE: Final = 6
    if True:
        NESTED_FUNCTION: Final = 7
class Color(Enum):
    RED = 1
    label: Final = "red"
    if True:
        BLUE = 2
class Number(enum.IntEnum):
    ONE = 1
class Text(enum.StrEnum):
    VALUE = "value"
class Repr(enum.ReprEnum):
    VALUE = 1
class Bits(enum.Flag):
    ONE = 1
class IntBits(enum.IntFlag):
    ONE = 1
CHAINED = OTHER = 1
LEFT, RIGHT = (1, 2)
AUGMENTED += 1
for ITEM in items:
    pass
with manager() as RESOURCE:
    pass
target.ATTRIBUTE = 1
items[0] = 1
target.ANNOTATED: Final = 1
items[0]: Final = 1
import module as IMPORTED
from module import value as IMPORTED_VALUE
SUPPRESSED = 1  # noqa: GR004 -- external spelling
suppressed_variable: Final = 1  # noqa: GR004 -- framework state
DOUBLED_HASH = 1  ## noqa: GR004 -- external spelling
TRAILING_DIRECTIVE = 1  # external spelling  # noqa: GR004
BARE_DIRECTIVE = 1  # noqa# external spelling
HASHED_CODE_LIST = 1  # noqa: GR004#external spelling
SUPPRESSED_MULTILINE = (  # noqa: GR004 -- external spelling
    1,
)
_suppressed_multiline: Final = (  # noqa: GR004, GR011 -- framework state
    "debug",
)
def read_suppressed_multiline():
    return _suppressed_multiline
"#,
    )
    .expect("allowed source should be written");
    fs::write(
        directory.join("allowed.pyi"),
        "from typing import Final\nSTUB_VALUE: Final\n_PRIVATE_STUB: Final[int]\n",
    )
    .expect("stub source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR004,GR011",
            "--output-format",
            "concise",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn follows_ruff_selector_specificity() {
    let directory = create_temp_directory("selectors");
    let path = directory.join("finding.py");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\nignore = [\"GR001\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--select", "GR", path.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("GR001"));
    assert!(output.stderr.is_empty());

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR001",
            "--ignore",
            "GR",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));

    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\n",
    )
    .expect("test configuration should be written");
    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--ignore", "GR", path.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No rules are enabled"));

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--output-format",
            "json",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"[]\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No rules are enabled"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_hidden_files_once() {
    let directory = create_temp_directory("discovery");
    let path = directory.join(".hidden.py");
    let excluded_path = directory.join(".git").join("hooks").join("excluded.py");
    let build_path = directory.join("build").join("included.py");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");
    fs::create_dir_all(excluded_path.parent().unwrap())
        .expect("excluded directory should be created");
    fs::write(&excluded_path, "def _save(path):\n    ...\n")
        .expect("excluded source should be written");
    fs::create_dir(build_path.parent().unwrap()).expect("build directory should be created");
    fs::write(&build_path, "def _paint(path):\n    ...\n").expect("build source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", directory.to_str().unwrap(), path.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .matches("GR001 Input")
            .count(),
        2
    );
    assert!(String::from_utf8_lossy(&output.stdout).ends_with("Found 2 findings.\n"));

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR001",
            excluded_path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("GR001"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn validates_per_file_globs_without_python_files() {
    let directory = create_temp_directory("invalid-glob");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\nper-file-ignores = { \"[\" = [\"GR001\"] }\n",
    )
    .expect("test configuration should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", directory.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid per-file ignore"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn formats_github_output() {
    let directory = create_temp_directory("github");
    let path = directory.join("finding.py");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR001",
            "--output-format",
            "github",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line=1,col=11,endLine=1,endColumn=15"));
    assert!(stdout.contains("Input `path` must be positional-only or keyword-only"));
    assert!(output.stderr.is_empty());

    fs::write(&path, "value = \"\"\"unterminated\nstring\n")
        .expect("invalid source should be written");
    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR001",
            "--output-format",
            "github",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line=1,endLine=3::"));
    assert!(!stdout.contains("line=1,col="));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn underlines_finding_ranges_in_full_output() {
    let directory = create_temp_directory("full-output-underline");
    fs::write(
        directory.join("test_multiline.py"),
        "def test_fetch():\n    try:\n        fetch()\n    except (\n        ValueError,\n        OSError,\n    ):\n        pass\n",
    )
    .expect("multi-line handler source should be written");
    fs::write(
        directory.join("test_singleline.py"),
        "def test_fetch():\n    try:\n        fetch()\n    except ValueError:\n        pass\n",
    )
    .expect("single-line handler source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR008",
            "--output-format",
            "full",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The range ends on the `):` line, so the underline covers the start line's tail.
    assert!(
        stdout.contains("  |\n4 |     except (\n  |     ^^^^^^^^ GR008\n"),
        "full output should underline `except (` to end of line, got:\n{stdout}"
    );
    // A same-row range keeps its exact width: `except ValueError` without the colon.
    assert!(
        stdout.contains("4 |     except ValueError:\n  |     ^^^^^^^^^^^^^^^^^ GR008\n"),
        "full output should underline the same-row range exactly, got:\n{stdout}"
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn aligns_full_output_carets_for_multi_digit_rows() {
    let directory = create_temp_directory("full-output-row-width");
    fs::write(
        directory.join("test_padded.py"),
        "def run(x: int) -> int:\n    a = 1\n    b = 2\n    c = 3\n    d = 4\n    e = 5\n    f = 6\n    g = 7\n    h = 8\n    i = 9\n    if not x:\n        return a\n    else:\n        return b\n",
    )
    .expect("padded source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR010",
            "--output-format",
            "full",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("   |\n11 |     if not x:\n   |     ^^^^^^^^ GR010\n"),
        "full output should widen the gutter to the row number, got:\n{stdout}"
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn expands_tabs_in_full_output() {
    let directory = create_temp_directory("full-output-tabs");
    fs::write(
        directory.join("test_tabs.py"),
        "def run(x: int) -> int:\n\tif not x:\n\t\treturn 1\n\telse:\n\t\treturn 2\n",
    )
    .expect("tab-indented source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR010",
            "--output-format",
            "full",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("2 |     if not x:\n  |     ^^^^^^^^ GR010\n"),
        "full output should expand the leading tab to four spaces, got:\n{stdout}"
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn measures_wide_characters_in_full_output() {
    let directory = create_temp_directory("full-output-wide");
    fs::write(
        directory.join("test_wide.py"),
        "def 日本語(x):\n    return x\n",
    )
    .expect("wide-character source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR005",
            "--output-format",
            "full",
            ".",
        ])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The prefix before the parameter spans eleven display columns, so padding by character count would mispoint.
    assert!(
        stdout.contains("1 | def 日本語(x):\n  |            ^ GR005\n"),
        "full output should pad by display width, got:\n{stdout}"
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn warns_when_nearest_configs_disable_all_rules() {
    let directory = create_temp_directory("nearest-config");
    let nested = directory.join("nested");
    fs::create_dir(&nested).expect("nested directory should be created");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\n",
    )
    .expect("root configuration should be written");
    fs::write(nested.join("pyproject.toml"), "[tool.gruff]\n")
        .expect("nested configuration should be written");
    fs::write(nested.join("clean.py"), "def run():\n    return 1\n")
        .expect("clean source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", directory.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No rules are enabled"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn supports_absolute_ignores_and_explicit_non_python_files() {
    let directory = create_temp_directory("absolute-ignore");
    let path = directory.join("policy.txt");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");
    let pattern = path.display().to_string().replace('\\', "\\\\");
    fs::write(
        directory.join("pyproject.toml"),
        format!(
            "[tool.gruff.lint]\nselect = [\"GR001\"]\nper-file-ignores = {{ \"{pattern}\" = [\"GR001\"] }}\n"
        ),
    )
    .expect("test configuration should be written");

    let ignored_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert!(ignored_output.status.success());
    assert_eq!(ignored_output.stdout, b"All checks passed!\n");

    let explicit_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR001",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(explicit_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&explicit_output.stdout).contains("GR001"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn respects_gitignore_except_for_explicit_files() {
    let directory = create_temp_directory("gitignore");
    let ignored_path = directory.join("ignored.py");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(directory.join(".gitignore"), "ignored.py\n").expect("ignore file should be written");
    fs::create_dir(directory.join(".git")).expect("git marker should be created");
    fs::write(&ignored_path, "def _load(path):\n    ...\n")
        .expect("ignored source should be written");

    let discovered_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", directory.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert!(discovered_output.status.success());
    assert_eq!(discovered_output.stdout, b"All checks passed!\n");
    assert!(discovered_output.stderr.is_empty());

    let explicit_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", ignored_path.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert_eq!(explicit_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&explicit_output.stdout).contains("GR001"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn distinguishes_syntax_and_configuration_failures() {
    let directory = create_temp_directory("failures");
    let invalid_path = directory.join("invalid.py");
    fs::write(&invalid_path, "def broken(\n").expect("invalid source should be written");

    let syntax_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "GR001",
            invalid_path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(syntax_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&syntax_output.stdout).contains("invalid-syntax"));

    let disabled_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--isolated", invalid_path.to_str().unwrap()])
        .output()
        .expect("gruff should run");
    assert_eq!(disabled_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&disabled_output.stdout).contains("invalid-syntax"));
    assert!(String::from_utf8_lossy(&disabled_output.stderr).contains("No rules are enabled"));

    let configuration_output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--isolated",
            "--select",
            "UNKNOWN",
            invalid_path.to_str().unwrap(),
        ])
        .output()
        .expect("gruff should run");
    assert_eq!(configuration_output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&configuration_output.stderr).contains("Unknown rule"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn supports_negated_per_file_ignores() {
    let directory = create_temp_directory("negated-ignore");
    for name in ["keep.py", "drop.py"] {
        fs::write(directory.join(name), "def _load(path):\n    ...\n")
            .expect("finding source should be written");
    }
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\nper-file-ignores = { \"!keep.py\" = [\"GR001\"] }\n",
    )
    .expect("test configuration should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["check", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("gruff should run");
    assert_eq!(output.status.code(), Some(1));
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert!(
        findings[0]["filename"]
            .as_str()
            .unwrap()
            .ends_with("keep.py")
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn resolves_explicit_config_patterns_from_current_directory() {
    let directory = create_temp_directory("explicit-config-root");
    let project = directory.join("project");
    let source_directory = project.join("src");
    let config_directory = directory.join("config");
    fs::create_dir_all(&source_directory).expect("source directory should be created");
    fs::create_dir(&config_directory).expect("config directory should be created");
    fs::write(
        config_directory.join("pyproject.toml"),
        "[tool.gruff.lint]\nselect = [\"GR001\"]\nper-file-ignores = { \"src/finding.py\" = [\"GR001\"] }\n",
    )
    .expect("test configuration should be written");
    fs::write(
        source_directory.join("finding.py"),
        "def _load(path):\n    ...\n",
    )
    .expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args([
            "check",
            "--config",
            config_directory.join("pyproject.toml").to_str().unwrap(),
            ".",
        ])
        .current_dir(&project)
        .output()
        .expect("gruff should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn binds_every_rule_to_its_doc_and_tables() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let directory = root.join("docs/rules");
    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "--all", "--output-format", "json"])
        .output()
        .expect("gruff should explain every rule");
    let rules: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");

    let documented: Vec<PathBuf> = fs::read_dir(&directory)
        .expect("rule doc directory should be readable")
        .map(|entry| entry.expect("rule document should be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    assert_eq!(documented.len(), rules.as_array().unwrap().len());

    let readme = fs::read_to_string(root.join("README.md")).expect("README should be readable");
    let index =
        fs::read_to_string(root.join("docs/index.md")).expect("site home should be readable");
    assert_eq!(
        readme.matches("\n| GR").count(),
        rules.as_array().unwrap().len()
    );
    assert_eq!(
        index.matches("\n| GR").count(),
        rules.as_array().unwrap().len()
    );

    for rule in rules.as_array().unwrap() {
        let code = rule["code"].as_str().unwrap();
        let name = rule["name"].as_str().unwrap();
        let summary = rule["summary"].as_str().unwrap();
        let document = fs::read_to_string(directory.join(format!("{name}.md")))
            .expect("every rule should have a rule document");
        assert_eq!(document, rule["explanation"].as_str().unwrap());
        assert!(
            document.starts_with(&format!("# {name} ({code})\n")),
            "unexpected title in {name}.md"
        );
        // Without the trailing newline, `--all` text output glues rules together.
        assert!(
            document.ends_with('\n'),
            "{name}.md should end with a newline"
        );
        assert!(
            readme.contains(&format!(
                "| {code} | [`{name}`](https://wkentaro.github.io/gruff/rules/{name}/) | {summary} |"
            )),
            "README.md should carry the {code} row"
        );
        assert!(
            index.contains(&format!(
                "| {code} | [`{name}`](rules/{name}.md) | {summary} |"
            )),
            "docs/index.md should carry the {code} row"
        );
        let sections: Vec<&str> = document
            .lines()
            .filter(|line| line.starts_with("## "))
            .collect();
        assert_eq!(
            sections,
            [
                "## What it does",
                "## Why",
                "## Example",
                "## When to suppress"
            ],
            "unexpected sections in {name}.md"
        );
    }
}

#[test]
fn explains_rule_by_code_and_name() {
    let by_code = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "GR004"])
        .output()
        .expect("gruff should explain a rule code");
    let by_name = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "final-constants"])
        .output()
        .expect("gruff should explain a rule name");

    assert!(by_code.status.success());
    assert!(by_code.stderr.is_empty());
    let explanation = String::from_utf8_lossy(&by_code.stdout);
    assert!(explanation.starts_with("# final-constants (GR004)\n"));
    assert!(explanation.contains("\n## When to suppress\n"));
    assert_eq!(by_code.stdout, by_name.stdout);

    let json = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "GR004", "--output-format", "json"])
        .output()
        .expect("gruff should explain a rule as JSON");
    assert!(json.status.success());
    let rule: Value = serde_json::from_slice(&json.stdout).expect("output should be JSON");
    let rule = rule.as_object().expect("one rule should be an object");
    // The parsed map is sorted, so this pins the key set rather than the emitted order.
    assert_eq!(
        rule.keys().collect::<Vec<_>>(),
        ["code", "explanation", "name", "summary"]
    );
    assert_eq!(rule["code"], "GR004");
    assert_eq!(rule["name"], "final-constants");
    assert_eq!(
        rule["explanation"].as_str().unwrap().as_bytes(),
        by_code.stdout
    );
}

#[test]
fn explains_all_rules() {
    let text = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "--all"])
        .output()
        .expect("gruff should explain every rule");
    let json = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "--all", "--output-format", "json"])
        .output()
        .expect("gruff should explain every rule as JSON");

    assert!(json.status.success());
    let rules: Value = serde_json::from_slice(&json.stdout).expect("output should be JSON");
    let rules = rules.as_array().expect("every rule should be an entry");
    assert!(!rules.is_empty());

    assert!(text.status.success());
    assert!(text.stderr.is_empty());
    let explanations = String::from_utf8_lossy(&text.stdout);
    assert_eq!(
        explanations.matches("\n## When to suppress\n").count(),
        rules.len()
    );
    assert!(explanations.contains("# no-non-public-docstrings (GR006)\n"));

    let final_constants = rules
        .iter()
        .find(|rule| rule["code"] == "GR004")
        .expect("GR004 should be explained");
    assert_eq!(final_constants["name"], "final-constants");
    assert_eq!(
        final_constants["summary"],
        "Uppercase names and `Final` annotations appear together."
    );
    assert!(
        final_constants["explanation"]
            .as_str()
            .unwrap()
            .starts_with("# final-constants (GR004)\n")
    );
}

#[test]
fn rejects_unknown_rule() {
    let output = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "GR999"])
        .output()
        .expect("gruff should run");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown rule: GR999"));
    assert!(stderr.contains("GR001"));

    // Codes and names are matched literally, so a lowercase code is not a rule.
    let lowercased = Command::new(env!("CARGO_BIN_EXE_gruff"))
        .args(["rule", "gr004"])
        .output()
        .expect("gruff should run");
    assert_eq!(lowercased.status.code(), Some(2));
}
