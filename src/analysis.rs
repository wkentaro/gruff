use ruff_python_ast::Expr;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::TextRange;

pub(crate) struct PrivateInput<'a> {
    pub(crate) name: &'a str,
    pub(crate) range: TextRange,
    pub(crate) is_positional_or_keyword: bool,
    pub(crate) is_required: bool,
}

pub(crate) fn classify_private_inputs<'a>(
    definition: &'a StmtFunctionDef,
    is_method: bool,
) -> Vec<PrivateInput<'a>> {
    let positional = definition
        .parameters
        .posonlyargs
        .iter()
        .map(|parameter| (parameter, false))
        .chain(
            definition
                .parameters
                .args
                .iter()
                .map(|parameter| (parameter, true)),
        );
    let receiver_count = usize::from(
        is_method && !is_static_method(definition) && positional.clone().next().is_some(),
    );

    positional
        .skip(receiver_count)
        .map(|(parameter, is_positional_or_keyword)| PrivateInput {
            name: parameter.name().as_str(),
            range: parameter.name().range,
            is_positional_or_keyword,
            is_required: parameter.default.is_none(),
        })
        .chain(
            definition
                .parameters
                .kwonlyargs
                .iter()
                .map(|parameter| PrivateInput {
                    name: parameter.name().as_str(),
                    range: parameter.name().range,
                    is_positional_or_keyword: false,
                    is_required: parameter.default.is_none(),
                }),
        )
        .collect()
}

pub(crate) fn find_private_definitions(statements: &[Stmt]) -> Vec<(&StmtFunctionDef, bool)> {
    let mut visitor = PrivateDefinitionVisitor {
        scope: DefinitionScope::Module,
        definitions: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.definitions
}

fn is_private_definition(name: &str) -> bool {
    name.starts_with('_') && !name.starts_with("__") && !name.ends_with('_')
}

fn is_static_method(definition: &StmtFunctionDef) -> bool {
    definition.decorator_list.iter().any(
        |decorator| matches!(&decorator.expression, Expr::Name(name) if name.id == "staticmethod"),
    )
}

#[derive(Clone, Copy)]
enum DefinitionScope {
    Module,
    Class,
    Function,
}

struct PrivateDefinitionVisitor<'a> {
    scope: DefinitionScope,
    definitions: Vec<(&'a StmtFunctionDef, bool)>,
}

impl<'a> Visitor<'a> for PrivateDefinitionVisitor<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        let previous_scope = self.scope;
        match statement {
            Stmt::FunctionDef(definition) => {
                match previous_scope {
                    DefinitionScope::Module if is_private_definition(&definition.name) => {
                        self.definitions.push((definition, false));
                    }
                    DefinitionScope::Class if is_private_definition(&definition.name) => {
                        self.definitions.push((definition, true));
                    }
                    DefinitionScope::Module
                    | DefinitionScope::Class
                    | DefinitionScope::Function => {}
                }
                self.scope = DefinitionScope::Function;
            }
            Stmt::ClassDef(_) => self.scope = DefinitionScope::Class,
            _ => {}
        }
        walk_stmt(self, statement);
        self.scope = previous_scope;
    }
}
