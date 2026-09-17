//! Track directive blocks on the whole Root, including comments outside statements.
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind as K,
    tree_sitter::{Node, Range},
};

pub(super) struct Influence {
    all: bool,
    intervals: Vec<std::ops::Range<usize>>,
}
impl Influence {
    pub fn affects(&self, range: &Range) -> bool {
        self.all
            || self
                .intervals
                .iter()
                .any(|i| i.start < range.end_byte && range.start_byte < i.end)
    }
}

pub(super) fn influence(root: &Node<'_>) -> Influence {
    let mut result = Influence {
        all: false,
        intervals: Vec::new(),
    };
    // (start, IF rather than BEGIN, ELSE already seen)
    let mut blocks: Vec<(usize, bool, bool)> = Vec::new();
    let tokens: Vec<_> = root
        .descendants()
        .filter(|n| n.node_or_token.as_token().is_some())
        .collect();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind() != K::C_COMMENT {
            continue;
        }
        let text = token.text();
        let Some(body) = text.strip_prefix("/*").and_then(|s| s.strip_suffix("*/")) else {
            continue;
        };
        let body = body.trim().strip_prefix('%').unwrap_or(body.trim()).trim();
        let mut words = body.split_whitespace();
        let keyword = words.next().unwrap_or("").to_ascii_uppercase();
        let has_argument = words.next().is_some();
        let range = token.range();
        match keyword.as_str() {
            "IF" if has_argument => blocks.push((range.start_byte, true, false)),
            "BEGIN" if !has_argument => blocks.push((range.start_byte, false, false)),
            "END" if !has_argument => {
                if let Some((start, _, _)) = blocks.pop() {
                    result.intervals.push(start..range.end_byte);
                } else {
                    result.all = true;
                }
            }
            "ELSE" | "ELIF" | "ELSEIF" => {
                if let Some((_, is_if, seen_else)) = blocks.last_mut() {
                    let is_else = keyword == "ELSE";
                    if !*is_if || *seen_else || (is_else == has_argument) {
                        result.all = true;
                    }
                    *seen_else |= is_else;
                } else {
                    result.all = true;
                }
            }
            "IF" | "BEGIN" | "END" => result.all = true,
            _ => {
                let replacement = body.starts_with(['#', '$']);
                let next = tokens[index + 1..]
                    .iter()
                    .find(|n| !matches!(n.kind(), K::C_COMMENT | K::SQL_COMMENT));
                // A single-line block immediately preceding a sample literal is the
                // same bind-comment shape accepted by the parser's transformer.
                let bind = !text.contains('\n')
                    && !body.starts_with('+')
                    && next.is_some_and(|n| {
                        n.range().start_byte == range.end_byte
                            && matches!(
                                n.kind(),
                                K::ICONST
                                    | K::FCONST
                                    | K::SCONST
                                    | K::TRUE_P
                                    | K::FALSE_P
                                    | K::NULL_P
                            )
                    });
                if replacement || bind {
                    let end = next
                        .filter(|n| n.kind() != K::Semicolon)
                        .map_or(range.end_byte, |n| n.range().end_byte);
                    result.intervals.push(range.start_byte..end);
                }
            }
        }
    }
    if let Some((start, _, _)) = blocks.first() {
        result.intervals.push(*start..root.range().end_byte);
    }
    result
}
