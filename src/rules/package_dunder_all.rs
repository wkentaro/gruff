use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use ruff_python_ast::CmpOp;
use ruff_python_ast::ElifElseClause;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::Pattern;
use ruff_python_ast::Singleton;
use ruff_python_ast::Stmt;
use ruff_python_ast::UnaryOp;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_expr;
use ruff_text_size::TextRange;

use super::Diagnostic;

pub(crate) const CODE: &str = "GR003";
pub(crate) const NAME: &str = "package-dunder-all";
pub(crate) const SUMMARY: &str = "Every public package import path defines `__all__`.";

const MAX_STATES: usize = 32;
const MAX_CONDITIONS: usize = 32;

pub(crate) fn check(path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    if !matches!(file_name, "__init__.py" | "__init__.pyi") {
        return Vec::new();
    }

    let is_stub = file_name.ends_with(".pyi");
    let range = analyze_suite(statements, vec![State::default()], is_stub)
        .into_iter()
        .filter(|state| state.flow == Flow::Normal)
        .filter_map(|state| state.find_bad_path_range())
        .min_by_key(|range| range.start());

    range
        .map(|range| Diagnostic {
            message:
                "Package initializer with public bindings must define __all__ on every import path"
                    .to_owned(),
            range,
            noqa_offset: None,
        })
        .into_iter()
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Flow {
    Normal,
    Break,
    Continue,
    Raised,
}

#[derive(Clone)]
struct State {
    history: Option<Rc<Operation>>,
    flow: Flow,
    exception: Option<String>,
    conditions: Vec<(ConditionKey, bool)>,
    type_checking_bindings: HashMap<String, TypeCheckingBinding>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            history: None,
            flow: Flow::Normal,
            exception: None,
            conditions: Vec::new(),
            type_checking_bindings: HashMap::new(),
        }
    }
}

impl State {
    fn add_operation(&mut self, kind: OperationKind) {
        self.history = Some(Rc::new(Operation {
            previous: self.history.clone(),
            kind,
        }));
    }

    fn bind(&mut self, binding: Binding<'_>) {
        self.conditions
            .retain(|(condition, _)| condition.name != binding.name);
        self.type_checking_bindings.remove(binding.name);
        self.add_operation(OperationKind::Bind {
            name: binding.name.to_owned(),
            range: binding.range,
        });
    }

    fn delete(&mut self, binding: Binding<'_>) {
        self.conditions
            .retain(|(condition, _)| condition.name != binding.name);
        self.type_checking_bindings.remove(binding.name);
        self.add_operation(OperationKind::Delete {
            name: binding.name.to_owned(),
        });
    }

    fn require(&mut self, binding: Binding<'_>) {
        self.add_operation(OperationKind::Require {
            name: binding.name.to_owned(),
        });
    }

    fn bind_type_checking(&mut self, binding: Binding<'_>, kind: TypeCheckingBinding) {
        self.bind(binding);
        self.type_checking_bindings
            .insert(binding.name.to_owned(), kind);
    }

    fn find_bad_path_range(&self) -> Option<TextRange> {
        let mut history = Vec::new();
        let mut operation = self.history.as_deref();
        while let Some(current) = operation {
            history.push(&current.kind);
            operation = current.previous.as_deref();
        }

        let mut bindings = HashMap::new();
        for operation in history.into_iter().rev() {
            match operation {
                OperationKind::Bind { name, range } => {
                    bindings.entry(name.as_str()).or_insert(*range);
                }
                OperationKind::Delete { name } => {
                    bindings.remove(name.as_str())?;
                }
                OperationKind::Require { name } => {
                    bindings.get(name.as_str())?;
                }
            }
        }
        if bindings.contains_key("__all__") {
            return None;
        }

        bindings
            .into_iter()
            .filter(|(name, _)| !name.starts_with('_'))
            .map(|(_, range)| range)
            .min_by_key(|range| range.start())
    }
}

#[derive(Clone, Copy)]
enum TypeCheckingBinding {
    Module,
    Flag,
}

struct Operation {
    previous: Option<Rc<Operation>>,
    kind: OperationKind,
}

enum OperationKind {
    Bind { name: String, range: TextRange },
    Delete { name: String },
    Require { name: String },
}

#[derive(Clone, Copy)]
struct Binding<'a> {
    name: &'a str,
    range: TextRange,
}

fn analyze_suite(statements: &[Stmt], mut states: Vec<State>, is_stub: bool) -> Vec<State> {
    for statement in statements {
        let (normal, mut carried): (Vec<_>, Vec<_>) = states
            .into_iter()
            .partition(|state| state.flow == Flow::Normal);
        carried.extend(analyze_statement(statement, normal, is_stub));
        carried.truncate(MAX_STATES);
        states = carried;
    }
    states
}

fn analyze_statement(statement: &Stmt, mut states: Vec<State>, is_stub: bool) -> Vec<State> {
    match statement {
        Stmt::FunctionDef(definition) => {
            bind_function_header(&mut states, definition);
            bind_states(
                &mut states,
                Binding {
                    name: definition.name.as_str(),
                    range: definition.name.range,
                },
            );
        }
        Stmt::ClassDef(class) => {
            bind_class_header(&mut states, class);
            bind_states(
                &mut states,
                Binding {
                    name: class.name.as_str(),
                    range: class.name.range,
                },
            );
        }
        Stmt::Delete(deletion) => {
            for target in &deletion.targets {
                delete_target(&mut states, target);
            }
        }
        Stmt::TypeAlias(alias) => bind_target(&mut states, &alias.name),
        Stmt::Assign(assignment) => {
            bind_expression(&mut states, &assignment.value);
            for target in &assignment.targets {
                bind_assignment_target(&mut states, target);
            }
        }
        Stmt::AugAssign(assignment) => {
            evaluate_augmented_target(&mut states, &assignment.target);
            bind_expression(&mut states, &assignment.value);
        }
        Stmt::AnnAssign(assignment) => {
            if let Some(value) = &assignment.value {
                bind_expression(&mut states, value);
            }
            evaluate_assignment_target(&mut states, &assignment.target);
            if assignment.value.is_some() || is_stub {
                bind_target(&mut states, &assignment.target);
            }
        }
        Stmt::For(statement) => return analyze_for(statement, states, is_stub),
        Stmt::While(statement) => return analyze_while(statement, states, is_stub),
        Stmt::If(statement) => {
            return analyze_if(
                &statement.test,
                &statement.body,
                &statement.elif_else_clauses,
                states,
                is_stub,
            );
        }
        Stmt::With(statement) => {
            for item in &statement.items {
                bind_expression(&mut states, &item.context_expr);
                if let Some(target) = &item.optional_vars {
                    bind_assignment_target(&mut states, target);
                }
            }
            return analyze_suite(&statement.body, states, is_stub);
        }
        Stmt::Match(statement) => return analyze_match(statement, states, is_stub),
        Stmt::Raise(statement) => {
            if let Some(exception) = &statement.exc {
                bind_expression(&mut states, exception);
            }
            if let Some(cause) = &statement.cause {
                bind_expression(&mut states, cause);
            }
            for state in &mut states {
                state.flow = Flow::Raised;
                state.exception = statement.exc.as_deref().and_then(get_raised_exception_name);
            }
        }
        Stmt::Try(statement) => return analyze_try(statement, states, is_stub),
        Stmt::Assert(statement) => {
            bind_expression(&mut states, &statement.test);
            let (passed, mut failed) = branch_states(&statement.test, states);
            if let Some(message) = &statement.msg {
                bind_expression(&mut failed, message);
            }
            for state in &mut failed {
                state.flow = Flow::Raised;
                state.exception = Some("AssertionError".to_owned());
            }
            failed.extend(passed);
            failed.truncate(MAX_STATES);
            return failed;
        }
        Stmt::Import(statement) => {
            for alias in &statement.names {
                let binding = alias.asname.as_ref().map_or_else(
                    || Binding {
                        name: alias.name.as_str().split('.').next().unwrap_or_default(),
                        range: alias.name.range,
                    },
                    |name| Binding {
                        name: name.as_str(),
                        range: name.range,
                    },
                );
                if alias.name.as_str() == "typing" || alias.name.as_str() == "typing_extensions" {
                    bind_type_checking_states(&mut states, binding, TypeCheckingBinding::Module);
                } else {
                    bind_states(&mut states, binding);
                }
            }
        }
        Stmt::ImportFrom(statement) => {
            if statement.level == 0
                && statement
                    .module
                    .as_ref()
                    .is_some_and(|module| module == "__future__")
            {
                return states;
            }
            for alias in &statement.names {
                if alias.name.as_str() == "*" {
                    continue;
                }
                let name = alias.asname.as_ref().unwrap_or(&alias.name);
                let binding = Binding {
                    name: name.as_str(),
                    range: name.range,
                };
                if statement.level == 0
                    && matches!(
                        statement.module.as_ref().map(|module| module.as_str()),
                        Some("typing" | "typing_extensions")
                    )
                    && alias.name.as_str() == "TYPE_CHECKING"
                {
                    bind_type_checking_states(&mut states, binding, TypeCheckingBinding::Flag);
                } else {
                    bind_states(&mut states, binding);
                }
            }
        }
        Stmt::Expr(statement) => bind_expression(&mut states, &statement.value),
        Stmt::Break(_) => {
            for state in &mut states {
                state.flow = Flow::Break;
            }
        }
        Stmt::Continue(_) => {
            for state in &mut states {
                state.flow = Flow::Continue;
            }
        }
        Stmt::Return(_)
        | Stmt::Global(_)
        | Stmt::Nonlocal(_)
        | Stmt::Pass(_)
        | Stmt::IpyEscapeCommand(_) => {}
    }
    states
}

fn analyze_for(
    statement: &ruff_python_ast::StmtFor,
    mut states: Vec<State>,
    is_stub: bool,
) -> Vec<State> {
    bind_expression(&mut states, &statement.iter);
    let mut entered = states.clone();
    bind_assignment_target(&mut entered, &statement.target);
    let entered = analyze_suite(&statement.body, entered, is_stub);
    let mut result = analyze_suite(&statement.orelse, states, is_stub);

    for mut state in entered {
        match state.flow {
            Flow::Normal | Flow::Continue => {
                state.flow = Flow::Normal;
                result.extend(analyze_suite(&statement.orelse, vec![state], is_stub));
            }
            Flow::Break => {
                state.flow = Flow::Normal;
                result.push(state);
            }
            Flow::Raised => result.push(state),
        }
        result.truncate(MAX_STATES);
    }
    result
}

fn analyze_while(
    statement: &ruff_python_ast::StmtWhile,
    mut states: Vec<State>,
    is_stub: bool,
) -> Vec<State> {
    bind_expression(&mut states, &statement.test);
    let (entered, skipped) = branch_states(&statement.test, states);
    let mut result = analyze_suite(&statement.orelse, skipped, is_stub);

    for mut state in analyze_suite(&statement.body, entered, is_stub) {
        match state.flow {
            Flow::Break => {
                state.flow = Flow::Normal;
                result.push(state);
            }
            Flow::Normal | Flow::Continue => {
                state.flow = Flow::Normal;
                bind_expression(std::slice::from_mut(&mut state), &statement.test);
                let (_, exited) = branch_states(&statement.test, vec![state]);
                result.extend(analyze_suite(&statement.orelse, exited, is_stub));
            }
            Flow::Raised => result.push(state),
        }
        result.truncate(MAX_STATES);
    }
    result
}

fn analyze_if(
    test: &Expr,
    body: &[Stmt],
    clauses: &[ElifElseClause],
    mut states: Vec<State>,
    is_stub: bool,
) -> Vec<State> {
    bind_expression(&mut states, test);
    let (matched, skipped) = branch_states(test, states);
    let mut result = analyze_suite(body, matched, is_stub);
    result.extend(analyze_clauses(clauses, skipped, is_stub));
    result.truncate(MAX_STATES);
    result
}

fn analyze_clauses(clauses: &[ElifElseClause], states: Vec<State>, is_stub: bool) -> Vec<State> {
    let Some((clause, remaining)) = clauses.split_first() else {
        return states;
    };
    if let Some(test) = &clause.test {
        analyze_if(test, &clause.body, remaining, states, is_stub)
    } else {
        analyze_suite(&clause.body, states, is_stub)
    }
}

fn analyze_match(
    statement: &ruff_python_ast::StmtMatch,
    mut states: Vec<State>,
    is_stub: bool,
) -> Vec<State> {
    bind_expression(&mut states, &statement.subject);
    let mut result = Vec::new();
    let mut remaining = states;

    for case in &statement.cases {
        let mut matched = remaining.clone();
        let mut unmatched = if case.pattern.is_irrefutable() {
            Vec::new()
        } else {
            remaining
        };
        bind_pattern(&mut matched, &case.pattern);
        if let Some(guard) = &case.guard {
            bind_expression(&mut matched, guard);
            let (accepted, rejected) = branch_states(guard, matched);
            matched = accepted;
            unmatched.extend(rejected);
            unmatched.truncate(MAX_STATES);
        }
        result.extend(analyze_suite(&case.body, matched, is_stub));
        result.truncate(MAX_STATES);
        remaining = unmatched;
        if remaining.is_empty() {
            break;
        }
    }
    result.extend(remaining);
    result.truncate(MAX_STATES);
    result
}

fn analyze_try(
    statement: &ruff_python_ast::StmtTry,
    states: Vec<State>,
    is_stub: bool,
) -> Vec<State> {
    let attempted = analyze_suite(&statement.body, states, is_stub);
    let mut result = Vec::new();
    let mut raised = Vec::new();
    for state in attempted {
        match state.flow {
            Flow::Normal => {
                result.extend(analyze_suite(&statement.orelse, vec![state], is_stub));
            }
            Flow::Raised => raised.push(state),
            Flow::Break | Flow::Continue => result.push(state),
        }
    }

    let mut handler_inputs = vec![Vec::new(); statement.handlers.len()];
    let mut uncaught = Vec::new();
    for mut state in raised {
        let mut selected = None;
        for (index, handler) in statement.handlers.iter().enumerate() {
            let ExceptHandler::ExceptHandler(handler) = handler;
            if let Some(type_) = &handler.type_ {
                bind_expression(std::slice::from_mut(&mut state), type_);
            }
            if does_handler_catch(handler.type_.as_deref(), state.exception.as_deref()) {
                selected = Some(index);
                break;
            }
        }
        if let Some(index) = selected {
            handler_inputs[index].push(state);
        } else {
            uncaught.push(state);
        }
    }

    for (handler, mut handled) in statement.handlers.iter().zip(handler_inputs) {
        let ExceptHandler::ExceptHandler(handler) = handler;
        for state in &mut handled {
            state.flow = Flow::Normal;
            state.exception = None;
        }
        if let Some(name) = &handler.name {
            bind_states(
                &mut handled,
                Binding {
                    name: name.as_str(),
                    range: name.range,
                },
            );
        }
        let mut handled = analyze_suite(&handler.body, handled, is_stub);
        if let Some(name) = &handler.name {
            delete_states(
                &mut handled,
                Binding {
                    name: name.as_str(),
                    range: name.range,
                },
            );
        }
        result.extend(handled);
        result.truncate(MAX_STATES);
    }
    result.extend(uncaught);
    result.truncate(MAX_STATES);

    if statement.finalbody.is_empty() {
        return result;
    }
    apply_finally(&statement.finalbody, result, is_stub)
}

fn apply_finally(statements: &[Stmt], states: Vec<State>, is_stub: bool) -> Vec<State> {
    let mut result = Vec::new();
    for mut state in states {
        let prior_flow = state.flow;
        state.flow = Flow::Normal;
        for mut finalized in analyze_suite(statements, vec![state], is_stub) {
            if finalized.flow == Flow::Normal {
                finalized.flow = prior_flow;
            }
            result.push(finalized);
            result.truncate(MAX_STATES);
        }
    }
    result
}

#[derive(Clone, Copy)]
enum Condition {
    Always,
    Never,
    Unknown,
}

#[derive(Clone, PartialEq)]
struct ConditionKey {
    name: String,
    identity: Option<Singleton>,
}

fn classify_condition(expression: &Expr, state: &State) -> Condition {
    match expression {
        Expr::BooleanLiteral(literal) => {
            if literal.value {
                Condition::Always
            } else {
                Condition::Never
            }
        }
        Expr::Name(name)
            if name.id == "TYPE_CHECKING"
                || matches!(
                    state.type_checking_bindings.get(name.id.as_str()),
                    Some(TypeCheckingBinding::Flag)
                ) =>
        {
            Condition::Never
        }
        Expr::Attribute(attribute)
            if attribute.attr.as_str() == "TYPE_CHECKING"
                && matches!(
                    attribute.value.as_ref(),
                    Expr::Name(name)
                        if matches!(
                            state.type_checking_bindings.get(name.id.as_str()),
                            Some(TypeCheckingBinding::Module)
                        )
                ) =>
        {
            Condition::Never
        }
        Expr::UnaryOp(unary) if unary.op == UnaryOp::Not => {
            match classify_condition(&unary.operand, state) {
                Condition::Always => Condition::Never,
                Condition::Never => Condition::Always,
                Condition::Unknown => Condition::Unknown,
            }
        }
        _ => Condition::Unknown,
    }
}

fn branch_states(expression: &Expr, states: Vec<State>) -> (Vec<State>, Vec<State>) {
    let mut matched = Vec::new();
    let mut skipped = Vec::new();
    for state in states {
        match classify_condition(expression, &state) {
            Condition::Always => matched.push(state),
            Condition::Never => skipped.push(state),
            Condition::Unknown => {
                branch_unknown_condition(expression, state, &mut matched, &mut skipped)
            }
        }
    }
    matched.truncate(MAX_STATES);
    skipped.truncate(MAX_STATES);
    (matched, skipped)
}

fn branch_unknown_condition(
    expression: &Expr,
    state: State,
    matched: &mut Vec<State>,
    skipped: &mut Vec<State>,
) {
    let Some((key, polarity)) = get_condition_key(expression) else {
        matched.push(state.clone());
        skipped.push(state);
        return;
    };
    if let Some((_, value)) = state
        .conditions
        .iter()
        .find(|(existing, _)| existing == &key)
    {
        if *value == polarity {
            matched.push(state);
        } else {
            skipped.push(state);
        }
        return;
    }

    let mut matched_state = state.clone();
    if matched_state.conditions.len() == MAX_CONDITIONS {
        return;
    }
    matched_state.conditions.push((key.clone(), polarity));
    matched.push(matched_state);
    let mut skipped_state = state;
    skipped_state.conditions.push((key, !polarity));
    skipped.push(skipped_state);
}

fn get_raised_exception_name(expression: &Expr) -> Option<String> {
    if let Expr::Call(call) = expression {
        return get_exception_class_name(&call.func);
    }
    get_exception_class_name(expression)
}

fn get_exception_class_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => get_exception_class_name(&attribute.value)
            .map(|value| format!("{value}.{}", attribute.attr)),
        _ => None,
    }
}

fn does_handler_catch(handler: Option<&Expr>, exception: Option<&str>) -> bool {
    let Some(handler) = handler else {
        return true;
    };
    let Some(exception) = exception else {
        return false;
    };
    match handler {
        Expr::Name(name) => name.id == exception,
        Expr::Attribute(_) => get_exception_class_name(handler).as_deref() == Some(exception),
        Expr::Named(named) => does_handler_catch(Some(&named.value), Some(exception)),
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .any(|handler| does_handler_catch(Some(handler), Some(exception))),
        _ => false,
    }
}

fn get_condition_key(expression: &Expr) -> Option<(ConditionKey, bool)> {
    match expression {
        Expr::Name(name) if name.id != "TYPE_CHECKING" => Some((
            ConditionKey {
                name: name.id.to_string(),
                identity: None,
            },
            true,
        )),
        Expr::UnaryOp(unary) if unary.op == UnaryOp::Not => {
            get_condition_key(&unary.operand).map(|(key, polarity)| (key, !polarity))
        }
        Expr::Compare(comparison)
            if comparison.ops.len() == 1 && comparison.comparators.len() == 1 =>
        {
            let polarity = match comparison.ops[0] {
                CmpOp::Is => true,
                CmpOp::IsNot => false,
                _ => return None,
            };
            get_identity_key(&comparison.left, &comparison.comparators[0])
                .map(|key| (key, polarity))
        }
        _ => None,
    }
}

fn get_identity_key(left: &Expr, right: &Expr) -> Option<ConditionKey> {
    let (name, value) = match (left, right) {
        (Expr::Name(name), value) | (value, Expr::Name(name)) => (name, value),
        _ => return None,
    };
    let value = match value {
        Expr::BooleanLiteral(literal) => Singleton::from(literal.value),
        Expr::NoneLiteral(_) => Singleton::None,
        _ => return None,
    };
    Some(ConditionKey {
        name: name.id.to_string(),
        identity: Some(value),
    })
}

fn bind_states(states: &mut [State], binding: Binding<'_>) {
    for state in states {
        state.bind(binding);
    }
}

fn bind_type_checking_states(
    states: &mut [State],
    binding: Binding<'_>,
    kind: TypeCheckingBinding,
) {
    for state in states {
        state.bind_type_checking(binding, kind);
    }
}

fn delete_states(states: &mut [State], binding: Binding<'_>) {
    for state in states {
        state.delete(binding);
    }
}

fn bind_target(states: &mut [State], target: &Expr) {
    for binding in collect_target_bindings(target) {
        bind_states(states, binding);
    }
}

fn bind_assignment_target(states: &mut [State], target: &Expr) {
    match target {
        Expr::Name(name) => bind_states(
            states,
            Binding {
                name: name.id.as_str(),
                range: name.range,
            },
        ),
        Expr::List(list) => {
            for element in &list.elts {
                bind_assignment_target(states, element);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                bind_assignment_target(states, element);
            }
        }
        Expr::Starred(starred) => bind_assignment_target(states, &starred.value),
        Expr::Attribute(attribute) => bind_expression(states, &attribute.value),
        Expr::Subscript(subscript) => {
            bind_expression(states, &subscript.value);
            bind_expression(states, &subscript.slice);
        }
        _ => {}
    }
}

fn evaluate_assignment_target(states: &mut [State], target: &Expr) {
    match target {
        Expr::List(list) => {
            for element in &list.elts {
                evaluate_assignment_target(states, element);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                evaluate_assignment_target(states, element);
            }
        }
        Expr::Starred(starred) => evaluate_assignment_target(states, &starred.value),
        Expr::Attribute(attribute) => bind_expression(states, &attribute.value),
        Expr::Subscript(subscript) => {
            bind_expression(states, &subscript.value);
            bind_expression(states, &subscript.slice);
        }
        _ => {}
    }
}

fn delete_target(states: &mut [State], target: &Expr) {
    match target {
        Expr::Name(name) => delete_states(
            states,
            Binding {
                name: name.id.as_str(),
                range: name.range,
            },
        ),
        Expr::List(list) => {
            for element in &list.elts {
                delete_target(states, element);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                delete_target(states, element);
            }
        }
        Expr::Starred(starred) => delete_target(states, &starred.value),
        Expr::Attribute(attribute) => bind_expression(states, &attribute.value),
        Expr::Subscript(subscript) => {
            bind_expression(states, &subscript.value);
            bind_expression(states, &subscript.slice);
        }
        _ => {}
    }
}

fn evaluate_augmented_target(states: &mut [State], target: &Expr) {
    match target {
        Expr::Name(name) => {
            let binding = Binding {
                name: name.id.as_str(),
                range: name.range,
            };
            for state in states {
                state.require(binding);
            }
        }
        Expr::Attribute(attribute) => bind_expression(states, &attribute.value),
        Expr::Subscript(subscript) => {
            bind_expression(states, &subscript.value);
            bind_expression(states, &subscript.slice);
        }
        _ => {}
    }
}

fn collect_target_bindings(target: &Expr) -> Vec<Binding<'_>> {
    let mut bindings = Vec::new();
    collect_target_bindings_into(target, &mut bindings);
    bindings
}

fn collect_target_bindings_into<'a>(target: &'a Expr, bindings: &mut Vec<Binding<'a>>) {
    match target {
        Expr::Name(name) => bindings.push(Binding {
            name: name.id.as_str(),
            range: name.range,
        }),
        Expr::List(list) => {
            for element in &list.elts {
                collect_target_bindings_into(element, bindings);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                collect_target_bindings_into(element, bindings);
            }
        }
        Expr::Starred(starred) => collect_target_bindings_into(&starred.value, bindings),
        _ => {}
    }
}

fn bind_expression(states: &mut [State], expression: &Expr) {
    let mut visitor = ExpressionBindingVisitor {
        bindings: Vec::new(),
    };
    visitor.visit_expr(expression);
    for binding in visitor.bindings {
        bind_states(states, binding);
    }
}

fn bind_function_header(states: &mut [State], definition: &ruff_python_ast::StmtFunctionDef) {
    let mut visitor = ExpressionBindingVisitor {
        bindings: Vec::new(),
    };
    for decorator in &definition.decorator_list {
        visitor.visit_decorator(decorator);
    }
    if let Some(type_params) = &definition.type_params {
        visitor.visit_type_params(type_params);
    }
    visitor.visit_parameters(&definition.parameters);
    if let Some(returns) = &definition.returns {
        visitor.visit_annotation(returns);
    }
    for binding in visitor.bindings {
        bind_states(states, binding);
    }
}

fn bind_class_header(states: &mut [State], class: &ruff_python_ast::StmtClassDef) {
    let mut visitor = ExpressionBindingVisitor {
        bindings: Vec::new(),
    };
    for decorator in &class.decorator_list {
        visitor.visit_decorator(decorator);
    }
    if let Some(type_params) = &class.type_params {
        visitor.visit_type_params(type_params);
    }
    if let Some(arguments) = &class.arguments {
        visitor.visit_arguments(arguments);
    }
    for binding in visitor.bindings {
        bind_states(states, binding);
    }
}

struct ExpressionBindingVisitor<'a> {
    bindings: Vec<Binding<'a>>,
}

impl<'a> Visitor<'a> for ExpressionBindingVisitor<'a> {
    fn visit_expr(&mut self, expression: &'a Expr) {
        match expression {
            Expr::Named(named) => {
                self.visit_expr(&named.value);
                collect_target_bindings_into(&named.target, &mut self.bindings);
            }
            Expr::Lambda(lambda) => {
                if let Some(parameters) = &lambda.parameters {
                    self.visit_parameters(parameters);
                }
            }
            Expr::ListComp(_) | Expr::SetComp(_) | Expr::DictComp(_) | Expr::Generator(_) => {}
            _ => walk_expr(self, expression),
        }
    }
}

fn bind_pattern(states: &mut [State], pattern: &Pattern) {
    let mut bindings = Vec::new();
    collect_pattern_bindings(pattern, &mut bindings);
    for binding in bindings {
        bind_states(states, binding);
    }
}

fn collect_pattern_bindings<'a>(pattern: &'a Pattern, bindings: &mut Vec<Binding<'a>>) {
    match pattern {
        Pattern::MatchSequence(sequence) => {
            for pattern in &sequence.patterns {
                collect_pattern_bindings(pattern, bindings);
            }
        }
        Pattern::MatchMapping(mapping) => {
            for pattern in &mapping.patterns {
                collect_pattern_bindings(pattern, bindings);
            }
            if let Some(name) = &mapping.rest {
                bindings.push(Binding {
                    name: name.as_str(),
                    range: name.range,
                });
            }
        }
        Pattern::MatchClass(class) => {
            for pattern in &class.arguments.patterns {
                collect_pattern_bindings(pattern, bindings);
            }
            for keyword in &class.arguments.keywords {
                collect_pattern_bindings(&keyword.pattern, bindings);
            }
        }
        Pattern::MatchStar(star) => {
            if let Some(name) = &star.name {
                bindings.push(Binding {
                    name: name.as_str(),
                    range: name.range,
                });
            }
        }
        Pattern::MatchAs(pattern) => {
            if let Some(inner) = &pattern.pattern {
                collect_pattern_bindings(inner, bindings);
            }
            if let Some(name) = &pattern.name {
                bindings.push(Binding {
                    name: name.as_str(),
                    range: name.range,
                });
            }
        }
        Pattern::MatchOr(or) => {
            if let Some(first) = or.patterns.first() {
                collect_pattern_bindings(first, bindings);
            }
        }
        Pattern::MatchValue(_) | Pattern::MatchSingleton(_) => {}
    }
}
