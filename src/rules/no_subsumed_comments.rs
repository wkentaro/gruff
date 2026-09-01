use std::collections::HashSet;

use ruff_python_ast::token::Tokens;

use super::Diagnostic;
use crate::analysis::analyze_comments;
use crate::analysis::find_noqa_directive;

pub(crate) const CODE: &str = "GR007";
pub(crate) const NAME: &str = "no-subsumed-comments";
pub(crate) const SUMMARY: &str =
    "One-line comments state something beyond the statements they annotate.";

pub(crate) fn check(source: &str, tokens: &Tokens) -> Vec<Diagnostic> {
    // File headers narrate the module rather than the code below them, and the corpus evaluation
    // measured this floor.
    const HEADER_LINES: usize = 5;

    let analysis = analyze_comments(source, tokens);

    analysis
        .blocks
        .iter()
        .filter_map(|block| {
            let [comment] = block.lines.as_slice() else {
                return None;
            };
            if comment.line_index < HEADER_LINES || !comment.is_own_line {
                return None;
            }

            let body = comment.text.trim_start_matches('#').trim();
            let body =
                find_noqa_directive(body).map_or(body, |(index, _)| body[..index].trim_end());
            if !is_prose(body) {
                return None;
            }
            let words = get_content_words(body);
            if words.len() < 2 {
                return None;
            }

            let code_tokens =
                get_code_tokens(find_window_lines(&analysis.masked_lines, comment.line_index));
            is_subsumed(&words, &code_tokens).then(|| Diagnostic {
                message: "One-line comment restates the statement it annotates; delete it or state what the code cannot".to_owned(),
                range: comment.range,
                noqa_offset: Some(comment.range.end()),
            })
        })
        .collect()
}

fn find_window_lines(lines: &[String], comment_line_index: usize) -> impl Iterator<Item = &String> {
    // Blank lines between the comment and the first non-blank line are skipped entirely; blank
    // lines inside the fixed physical-line slice that follows consume slots of it. The corpus
    // evaluation measured this size.
    const WINDOW_LINES: usize = 4;

    lines
        .iter()
        .skip(comment_line_index + 1)
        .skip_while(|line| line.trim().is_empty())
        .take(WINDOW_LINES)
        .filter(|line| !line.trim().is_empty())
}

fn is_subsumed(words: &[String], code_tokens: &HashSet<String>) -> bool {
    words.iter().all(|word| {
        code_tokens.contains(word)
            || get_synonym(word).is_some_and(|synonym| code_tokens.contains(synonym))
    })
}

fn is_prose(body: &str) -> bool {
    const DIRECTIVE_PREFIXES: &[&str] = &[
        "noqa", "type:", "pragma", "ruff", "mypy", "pylint", "fmt:", "isort", "flake8", "nosec",
        "coding:", "coding=",
    ];

    if body.is_empty() || body.starts_with(['!', ':']) {
        return false;
    }
    let lowercase = body.to_ascii_lowercase();
    if DIRECTIVE_PREFIXES
        .iter()
        .any(|prefix| lowercase.starts_with(prefix))
    {
        return false;
    }
    let mut characters = body.chars();
    if characters
        .next()
        .is_some_and(|character| "-=".contains(character))
        && characters
            .next()
            .is_some_and(|character| "-=".contains(character))
    {
        return false;
    }
    !contains_parenthesized_comma(body) && !starts_annotation(body)
}

fn contains_parenthesized_comma(body: &str) -> bool {
    let mut rest = body;
    while let Some(opening) = rest.find('(') {
        rest = &rest[opening + 1..];
        let Some(closing) = rest.find(')') else {
            return false;
        };
        if rest[..closing].contains(',') {
            return true;
        }
        rest = &rest[closing + 1..];
    }
    false
}

fn starts_annotation(body: &str) -> bool {
    let mut characters = body.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    while characters
        .clone()
        .next()
        .is_some_and(|character| character.is_alphanumeric() || character == '_')
    {
        characters.next();
    }
    characters.as_str().trim_start().starts_with(':')
}

fn get_content_words(body: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "this", "that", "these", "those", "to", "of", "for", "in", "on", "at",
        "with", "and", "or", "is", "are", "be", "was", "were", "we", "it", "its", "from", "as",
        "by", "into", "if", "then", "so", "all", "any", "each", "when", "whether",
    ];

    split_words(body)
        .map(str::to_lowercase)
        .filter(|word| {
            !word.chars().all(|character| character.is_ascii_digit())
                && !STOPWORDS.contains(&word.as_str())
        })
        .collect()
}

fn get_code_tokens<'a>(lines: impl Iterator<Item = &'a String>) -> HashSet<String> {
    let mut tokens = HashSet::new();
    for line in lines {
        if line.contains('(') {
            tokens.insert("call".to_owned());
        }
        for word in split_words(line) {
            tokens.insert(word.to_lowercase());
            add_identifier_parts(word, &mut tokens);
        }
    }
    tokens
}

fn split_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
}

fn add_identifier_parts(word: &str, tokens: &mut HashSet<String>) {
    let characters: Vec<_> = word.chars().collect();
    let mut start = 0;
    for index in 1..characters.len() {
        let previous = characters[index - 1];
        let current = characters[index];
        let next = characters.get(index + 1);
        let starts_capital_part = current.is_ascii_uppercase()
            && (previous.is_ascii_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_ascii_uppercase()
                    && next.is_some_and(|next| next.is_ascii_lowercase())));
        // A digit ends a multi-letter acronym but stays glued to a single letter, so `HTTP2Server`
        // splits while `GetX2Value` keeps `x2`.
        let acronym_has_multiple_letters = index - start > 1;
        let starts_digit_after_acronym = current.is_ascii_digit()
            && previous.is_ascii_uppercase()
            && acronym_has_multiple_letters;
        if starts_capital_part || starts_digit_after_acronym {
            tokens.insert(
                characters[start..index]
                    .iter()
                    .collect::<String>()
                    .to_lowercase(),
            );
            start = index;
        }
    }
    tokens.insert(
        characters[start..]
            .iter()
            .collect::<String>()
            .to_lowercase(),
    );
}

fn get_synonym(word: &str) -> Option<&'static str> {
    match word {
        "check" | "checks" | "checking" => Some("if"),
        "loop" | "loops" | "looping" | "iterate" | "iterates" | "iterating" => Some("for"),
        "returns" | "returning" => Some("return"),
        _ => None,
    }
}
