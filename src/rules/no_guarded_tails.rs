use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

use super::Diagnostic;

pub(crate) const CODE: &str = "GR009";
pub(crate) const NAME: &str = "no-guarded-tails";
pub(crate) const SUMMARY: &str =
    "Trailing conditions invert into guards instead of nesting the rest of the body.";

pub(crate) fn check(source: &str, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut visitor = GuardedTailVisitor {
        source,
        diagnostics: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.diagnostics
}

// A short straight-line tail costs a reader nothing, so the gate is what the corpus evaluation
// measured: a suite long enough to lose its condition off the top of the screen, or one that
// branches again.
fn meets_size_gate(source: &str, suite: &[Stmt]) -> bool {
    const GATE_LINES: usize = 10;

    count_lines(source, suite) >= GATE_LINES || contains_if(suite)
}

// Physical lines, so interior comments and blank lines count toward the gate.
fn count_lines(source: &str, suite: &[Stmt]) -> usize {
    let (Some(first), Some(last)) = (suite.first(), suite.last()) else {
        return 0;
    };
    source[first.start().to_usize()..last.end().to_usize()]
        .lines()
        .count()
}

fn contains_if(suite: &[Stmt]) -> bool {
    let mut visitor = NestedIfVisitor { is_found: false };
    visitor.visit_body(suite);
    visitor.is_found
}

struct NestedIfVisitor {
    is_found: bool,
}

impl<'a> Visitor<'a> for NestedIfVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        self.is_found |= matches!(statement, Stmt::If(_));
        if !self.is_found {
            walk_stmt(self, statement);
        }
    }
}

struct GuardedTailVisitor<'a> {
    source: &'a str,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Visitor<'a> for GuardedTailVisitor<'_> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        // Only a direct child of a function or loop body can invert, since a guard has to leave the
        // enclosing suite. Keying on the body also leaves the inner of two nested trailing ifs
        // alone: it becomes a direct child only after the outer one is inverted.
        let body = match statement {
            Stmt::FunctionDef(definition) => Some(&definition.body),
            Stmt::For(loop_statement) => Some(&loop_statement.body),
            Stmt::While(loop_statement) => Some(&loop_statement.body),
            _ => None,
        };
        if let Some(Stmt::If(tail)) = body.and_then(|body| body.last())
            && tail.elif_else_clauses.is_empty()
            && meets_size_gate(self.source, &tail.body)
        {
            self.diagnostics.push(Diagnostic {
                message: "Trailing `if` nests the rest of the body in its condition; invert it into an early `return` or `continue` guard".to_owned(),
                range: TextRange::new(tail.start(), tail.test.end()),
                noqa_offset: Some(tail.start()),
            });
        }
        walk_stmt(self, statement);
    }
}
