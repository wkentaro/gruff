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
    ];
    assert_eq!(findings.as_array().unwrap().len(), expected.len());
    for (finding, (row, column, message)) in findings.as_array().unwrap().iter().zip(expected) {
        assert_eq!(finding["code"], "GR004");
        assert_eq!(finding["name"], "final-constants");
        assert_eq!(finding["location"]["row"], row);
        assert_eq!(finding["location"]["column"], column);
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
            "GR004",
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
