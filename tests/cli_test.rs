use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn create_temp_directory(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("ruffhouse-cli-test-{}-{name}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).expect("stale test directory should be removable");
    }
    fs::create_dir(&path).expect("test directory should be created");
    path
}

#[test]
fn describes_check_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "--help"])
        .output()
        .expect("ruffhouse should show help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Run Ruffhouse on the given files or directories"));
    assert!(
        stdout.contains(
            "--select <RULE_CODE>\n          Comma-separated list of rule codes to enable"
        )
    );
    assert!(stdout.contains("Path to a pyproject.toml configuration file"));
    assert!(output.stderr.is_empty());
}

#[test]
fn checks_config_suppression_json_and_exit_status() {
    let directory = create_temp_directory("config");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.ruffhouse]\noutput-format = \"json\"\n\n[tool.ruffhouse.lint]\nselect = [\"RH001\"]\nper-file-ignores = { \"ignored.py\" = [\"RH001\"], \"invalid.py\" = [\"RH001\"] }\n",
    )
    .expect("test configuration should be written");
    fs::write(directory.join("finding.py"), "def _load(path):\n    ...\n")
        .expect("finding source should be written");
    fs::write(
        directory.join("suppressed.py"),
        "def _save(path):  # noqa: RH001\n    ...\n",
    )
    .expect("suppressed source should be written");
    fs::write(directory.join("ignored.py"), "def _send(path):\n    ...\n")
        .expect("ignored source should be written");
    fs::write(directory.join("invalid.py"), "def broken(\n")
        .expect("invalid source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "."])
        .current_dir(&directory)
        .output()
        .expect("ruffhouse should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 2);
    assert_eq!(findings[0]["code"], "RH001");
    assert_eq!(findings[0]["name"], "keyword-only-private-inputs");
    assert_eq!(findings[0]["severity"], "error");
    assert_eq!(findings[0]["location"]["row"], 1);
    assert!(PathBuf::from(findings[0]["filename"].as_str().unwrap()).is_absolute());
    assert_eq!(findings[1]["code"], "invalid-syntax");
    assert_eq!(findings[1]["name"], "invalid-syntax");
    assert_eq!(findings[1]["severity"], "error");

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn checks_required_private_inputs_selection_and_suppression() {
    let directory = create_temp_directory("required-private-inputs");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.ruffhouse.lint]\nselect = [\"RH002\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(
        directory.join("finding.py"),
        "def _render(*, path=None):\n    ...\n",
    )
    .expect("finding source should be written");
    fs::write(
        directory.join("suppressed.pyi"),
        "def _load(*, path=None): ...  # noqa: RH002\n",
    )
    .expect("suppressed source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("ruffhouse should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let findings: Value = serde_json::from_slice(&output.stdout).expect("output should be JSON");
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["code"], "RH002");
    assert_eq!(findings[0]["name"], "required-private-inputs");
    assert_eq!(
        findings[0]["message"],
        "Private input `path` must be required"
    );

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn follows_ruff_selector_specificity() {
    let directory = create_temp_directory("selectors");
    let path = directory.join("finding.py");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\nignore = [\"RH001\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "--select", "RH", path.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("RH001"));
    assert!(output.stderr.is_empty());

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "RH001",
            "--ignore",
            "RH",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(output.status.code(), Some(1));

    fs::write(
        directory.join("pyproject.toml"),
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\n",
    )
    .expect("test configuration should be written");
    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "--ignore", "RH", path.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(String::from_utf8_lossy(&output.stderr).contains("No rules are enabled"));

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--output-format",
            "json",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
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
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");
    fs::create_dir_all(excluded_path.parent().unwrap())
        .expect("excluded directory should be created");
    fs::write(&excluded_path, "def _save(path):\n    ...\n")
        .expect("excluded source should be written");
    fs::create_dir(build_path.parent().unwrap()).expect("build directory should be created");
    fs::write(&build_path, "def _paint(path):\n    ...\n").expect("build source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", directory.to_str().unwrap(), path.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .matches("RH001 Private input")
            .count(),
        2
    );

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "RH001",
            excluded_path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("RH001"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn validates_per_file_globs_without_python_files() {
    let directory = create_temp_directory("invalid-glob");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\nper-file-ignores = { \"[\" = [\"RH001\"] }\n",
    )
    .expect("test configuration should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", directory.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid per-file ignore"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn formats_github_output() {
    let directory = create_temp_directory("github");
    let path = directory.join("finding.py");
    fs::write(&path, "def _load(path):\n    ...\n").expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "RH001",
            "--output-format",
            "github",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("line=1,col=11,endLine=1,endColumn=15"));
    assert!(stdout.contains("Private input `path` must be keyword-only"));
    assert!(output.stderr.is_empty());

    fs::write(&path, "value = \"\"\"unterminated\nstring\n")
        .expect("invalid source should be written");
    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "RH001",
            "--output-format",
            "github",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
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
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\n",
    )
    .expect("root configuration should be written");
    fs::write(nested.join("pyproject.toml"), "[tool.ruffhouse]\n")
        .expect("nested configuration should be written");
    fs::write(nested.join("clean.py"), "def run():\n    return 1\n")
        .expect("clean source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", directory.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
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
            "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\nper-file-ignores = {{ \"{pattern}\" = [\"RH001\"] }}\n"
        ),
    )
    .expect("test configuration should be written");

    let ignored_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert!(ignored_output.status.success());
    assert_eq!(ignored_output.stdout, b"All checks passed!\n");

    let explicit_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "RH001",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(explicit_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&explicit_output.stdout).contains("RH001"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn respects_gitignore_except_for_explicit_files() {
    let directory = create_temp_directory("gitignore");
    let ignored_path = directory.join("ignored.py");
    fs::write(
        directory.join("pyproject.toml"),
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\n",
    )
    .expect("test configuration should be written");
    fs::write(directory.join(".gitignore"), "ignored.py\n").expect("ignore file should be written");
    fs::create_dir(directory.join(".git")).expect("git marker should be created");
    fs::write(&ignored_path, "def _load(path):\n    ...\n")
        .expect("ignored source should be written");

    let discovered_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", directory.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert!(discovered_output.status.success());
    assert_eq!(discovered_output.stdout, b"All checks passed!\n");
    assert!(discovered_output.stderr.is_empty());

    let explicit_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", ignored_path.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(explicit_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&explicit_output.stdout).contains("RH001"));

    fs::remove_dir_all(directory).expect("test directory should be removed");
}

#[test]
fn distinguishes_syntax_and_configuration_failures() {
    let directory = create_temp_directory("failures");
    let invalid_path = directory.join("invalid.py");
    fs::write(&invalid_path, "def broken(\n").expect("invalid source should be written");

    let syntax_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "RH001",
            invalid_path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(syntax_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&syntax_output.stdout).contains("invalid-syntax"));

    let disabled_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "--isolated", invalid_path.to_str().unwrap()])
        .output()
        .expect("ruffhouse should run");
    assert_eq!(disabled_output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&disabled_output.stdout).contains("invalid-syntax"));
    assert!(String::from_utf8_lossy(&disabled_output.stderr).contains("No rules are enabled"));

    let configuration_output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--isolated",
            "--select",
            "UNKNOWN",
            invalid_path.to_str().unwrap(),
        ])
        .output()
        .expect("ruffhouse should run");
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
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\nper-file-ignores = { \"!keep.py\" = [\"RH001\"] }\n",
    )
    .expect("test configuration should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args(["check", "--output-format", "json", "."])
        .current_dir(&directory)
        .output()
        .expect("ruffhouse should run");
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
        "[tool.ruffhouse.lint]\nselect = [\"RH001\"]\nper-file-ignores = { \"src/finding.py\" = [\"RH001\"] }\n",
    )
    .expect("test configuration should be written");
    fs::write(
        source_directory.join("finding.py"),
        "def _load(path):\n    ...\n",
    )
    .expect("finding source should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_ruffhouse"))
        .args([
            "check",
            "--config",
            config_directory.join("pyproject.toml").to_str().unwrap(),
            ".",
        ])
        .current_dir(&project)
        .output()
        .expect("ruffhouse should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"All checks passed!\n");
    assert!(output.stderr.is_empty());

    fs::remove_dir_all(directory).expect("test directory should be removed");
}
