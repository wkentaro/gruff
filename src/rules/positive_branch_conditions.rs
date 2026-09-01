use std::path::Path;

use ruff_python_ast::CmpOp;
use ruff_python_ast::Expr;
use ruff_python_ast::Stmt;
use ruff_python_ast::UnaryOp;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

use super::Diagnostic;

pub(crate) const CODE: &str = "GR010";
pub(crate) const NAME: &str = "positive-branch-conditions";
pub(crate) const SUMMARY: &str =
    "Branch conditions state the positive form instead of negating around an `else`.";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut visitor = NegatedConditionVisitor {
        diagnostics: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.diagnostics
}

// Only the outermost operation decides: a negation buried under `and` or inside an operand has no
// swap that states it positively, and a chained comparison has no single operator to invert.
fn is_negated(test: &Expr) -> bool {
    match test {
        Expr::UnaryOp(unary) => unary.op == UnaryOp::Not,
        Expr::Compare(comparison) => matches!(
            comparison.ops.as_ref(),
            [CmpOp::IsNot | CmpOp::NotEq | CmpOp::NotIn]
        ),
        _ => false,
    }
}

struct NegatedConditionVisitor {
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Visitor<'a> for NegatedConditionVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if let Stmt::If(branch) = statement
            && let [clause] = branch.elif_else_clauses.as_slice()
            && clause.test.is_none()
            && is_negated(&branch.test)
        {
            self.diagnostics.push(Diagnostic {
                message: "Negated `if` condition with an `else`; test the positive form and swap the branches".to_owned(),
                range: TextRange::new(branch.start(), branch.test.end()),
                noqa_offset: Some(branch.start()),
            });
        }
        walk_stmt(self, statement);
    }
}
