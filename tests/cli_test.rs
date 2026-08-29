use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

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
    assert_eq!(findings.as_array().unwrap().len(), 2);
    assert_eq!(findings[0]["code"], "GR001");
    assert_eq!(findings[0]["name"], "explicit-non-public-input-conventions");
    assert_eq!(
        findings[0]["message"],
        "Input `path` must be positional-only or keyword-only"
    );
    assert_eq!(findings[0]["severity"], "error");
    assert_eq!(findings[0]["location"]["row"], 1);
    assert!(PathBuf::from(findings[0]["filename"].as_str().unwrap()).is_absolute());
    assert_eq!(findings[1]["code"], "invalid-syntax");
    assert_eq!(findings[1]["name"], "invalid-syntax");
    assert_eq!(findings[1]["severity"], "error");

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
