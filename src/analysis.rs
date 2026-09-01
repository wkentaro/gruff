use std::collections::HashSet;

use ruff_python_ast::Expr;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::token::Tokens;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;
use ruff_source_file::LineIndex;
use ruff_source_file::OneIndexed;
use ruff_source_file::UniversalNewlines;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;

pub(crate) struct CommentLine<'a> {
    pub(crate) line_index: usize,
    pub(crate) range: TextRange,
    pub(crate) text: &'a str,
    pub(crate) is_own_line: bool,
}

pub(crate) struct CommentBlock<'a> {
    pub(crate) lines: Vec<CommentLine<'a>>,
}

pub(crate) struct CommentAnalysis<'a> {
    pub(crate) blocks: Vec<CommentBlock<'a>>,
    // Blanks out every comment, plus string tokens that begin on an assert or comparison line, so
    // prose in the source cannot pass for code. Indexed by physical line, like a comment's line
    // index.
    pub(crate) masked_lines: Vec<String>,
}

pub(crate) fn analyze_comments<'a>(source: &'a str, tokens: &Tokens) -> CommentAnalysis<'a> {
    let index = LineIndex::from_source_text(source);
    let comparison_lines: HashSet<_> = tokens
        .iter()
        .filter(|token| {
            matches!(
                token.kind(),
                TokenKind::Assert
                    | TokenKind::EqEqual
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::Greater
                    | TokenKind::LessEqual
                    | TokenKind::GreaterEqual
            )
        })
        .map(|token| index.line_index(token.start()).to_zero_indexed())
        .collect();
    let mut stripped = source.as_bytes().to_vec();
    let mut comments = Vec::new();

    for token in tokens {
        let line_index = index.line_index(token.start()).to_zero_indexed();
        match token.kind() {
            TokenKind::Comment => {
                blank_range(&mut stripped, token.range());
                comments.push((line_index, token.range()));
            }
            TokenKind::String | TokenKind::FStringMiddle | TokenKind::TStringMiddle
                if comparison_lines.contains(&line_index) =>
            {
                blank_range(&mut stripped, token.range());
            }
            _ => {}
        }
    }

    let masked_lines = get_lines(stripped);
    let mut blocks: Vec<CommentBlock> = Vec::new();
    for (line_index, range) in comments {
        let comment = CommentLine {
            line_index,
            range,
            text: &source[range],
            // A `#` comment runs to end of line, so it owns its line when nothing but blanks
            // precedes it.
            is_own_line: source[index
                .line_start(OneIndexed::from_zero_indexed(line_index), source)
                .to_usize()..range.start().to_usize()]
                .trim()
                .is_empty(),
        };
        let extends_block = comment.is_own_line
            && blocks.last().is_some_and(|block| {
                let previous = block.lines.last().expect("comment blocks are non-empty");
                previous.is_own_line && previous.line_index + 1 == comment.line_index
            });
        if extends_block {
            blocks
                .last_mut()
                .expect("an extendable block exists")
                .lines
                .push(comment);
        } else {
            blocks.push(CommentBlock {
                lines: vec![comment],
            });
        }
    }

    CommentAnalysis {
        blocks,
        masked_lines,
    }
}

// Returns the index of the hash the directive parses from, so a caller can slice the prose in front
// of it. In a hash run that is the last hash, so the sliced prose keeps the earlier ones. A
// directive's hash must open the comment or follow whitespace or another hash, which keeps a hash
// inside a URL or a word from starting one.
pub(crate) fn find_noqa_directive(body: &str) -> Option<(usize, Option<&str>)> {
    body.match_indices('#').find_map(|(index, _)| {
        body[..index]
            .chars()
            .next_back()
            .is_none_or(|character| character.is_ascii_whitespace() || character == '#')
            .then(|| parse_noqa_rules(&body[index + 1..]))?
            .map(|rules| (index, rules))
    })
}

// A bare directive suppresses every rule; an explicit code list suppresses only the codes it
// names, so an empty list suppresses nothing.
fn parse_noqa_rules(directive: &str) -> Option<Option<&str>> {
    let directive = directive.trim_start();
    if !directive
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("noqa"))
    {
        return None;
    }
    let directive = &directive[4..];
    if directive.chars().next().is_some_and(|character| {
        character != ':' && character != '#' && !character.is_ascii_whitespace()
    }) {
        return None;
    }
    Some(directive.trim_start().strip_prefix(':'))
}

pub(crate) fn matches_noqa_rules(rules: Option<&str>, code: &str) -> bool {
    rules.is_none_or(|rules| {
        // Ruff and flake8 stop lexing codes at the first token that is not a code, so whatever
        // follows is prose rather than a further suppression.
        rules
            .split_once('#')
            .map_or(rules, |(codes, _)| codes)
            .split(|character: char| character == ',' || character.is_ascii_whitespace())
            .filter(|rule| !rule.is_empty())
            .take_while(|rule| is_rule_code(rule))
            .any(|rule| rule == code)
    })
}

// Ruff's lexer accepts only uppercase codes and flake8 compares them case-sensitively, so a
// lowercase spelling is prose to both and must not suppress here either.
fn is_rule_code(rule: &str) -> bool {
    let digits = rule.trim_start_matches(|character: char| character.is_ascii_uppercase());
    digits.len() < rule.len()
        && !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn blank_range(source: &mut [u8], range: TextRange) {
    for byte in &mut source[range.start().to_usize()..range.end().to_usize()] {
        if !matches!(*byte, b'\n' | b'\r') {
            *byte = b' ';
        }
    }
}

fn get_lines(source: Vec<u8>) -> Vec<String> {
    String::from_utf8(source)
        .expect("blanking token spans preserves UTF-8")
        .universal_newlines()
        .map(|line| line.as_str().to_owned())
        .collect()
}

pub(crate) struct Input<'a> {
    pub(crate) name: &'a str,
    pub(crate) range: TextRange,
    pub(crate) is_positional_or_keyword: bool,
    pub(crate) is_required: bool,
}

pub(crate) fn classify_inputs<'a>(
    definition: &'a StmtFunctionDef,
    is_method: bool,
) -> Vec<Input<'a>> {
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
        .map(|(parameter, is_positional_or_keyword)| Input {
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
                .map(|parameter| Input {
                    name: parameter.name().as_str(),
                    range: parameter.name().range,
                    is_positional_or_keyword: false,
                    is_required: parameter.default.is_none(),
                }),
        )
        .collect()
}

pub(crate) fn find_definitions(statements: &[Stmt]) -> Vec<(&StmtFunctionDef, bool)> {
    let mut visitor = DefinitionVisitor {
        scope: DefinitionScope::Module,
        definitions: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.definitions
}

pub(crate) fn find_non_public_definitions(statements: &[Stmt]) -> Vec<(&StmtFunctionDef, bool)> {
    find_definitions(statements)
        .into_iter()
        .filter(|(definition, _)| is_non_public_definition(&definition.name))
        .collect()
}

pub(crate) fn find_public_definitions(statements: &[Stmt]) -> Vec<(&StmtFunctionDef, bool)> {
    find_definitions(statements)
        .into_iter()
        .filter(|(definition, _)| !is_non_public_definition(&definition.name))
        .collect()
}

fn is_non_public_definition(name: &str) -> bool {
    name.starts_with('_') && !name.ends_with('_')
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
}

struct DefinitionVisitor<'a> {
    scope: DefinitionScope,
    definitions: Vec<(&'a StmtFunctionDef, bool)>,
}

impl<'a> Visitor<'a> for DefinitionVisitor<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            // Nothing written inside a body that only runs when called is itself a definition, so
            // that body is left unwalked.
            Stmt::FunctionDef(definition) => self
                .definitions
                .push((definition, matches!(self.scope, DefinitionScope::Class))),
            Stmt::ClassDef(_) => {
                let previous_scope = self.scope;
                self.scope = DefinitionScope::Class;
                walk_stmt(self, statement);
                self.scope = previous_scope;
            }
            _ => walk_stmt(self, statement),
        }
    }
}
