use itertools::Itertools;

use crate::{
    cst::{add_indent, AlignInfo, AlignedExpr, Comment, Location},
    error::UroboroSQLFmtError,
    util::{add_space_by_range, count_width, is_line_overflow, tab_size, trim_bind_param},
};

/// Represents an ARRAY expression: ARRAY[expr, expr, ...]
#[derive(Debug, Clone)]
pub(crate) struct ArrayExpr {
    /// The ARRAY keyword (case may vary based on settings)
    keyword: String,
    /// Elements inside the brackets
    elements: Vec<AlignedExpr>,
    /// Location of the entire expression
    loc: Location,
    /// Whether to force multi-line rendering
    force_multi_line: bool,
    /// Bind parameter (head comment)
    head_comment: Option<String>,
}

impl ArrayExpr {
    pub(crate) fn new(keyword: String, elements: Vec<AlignedExpr>, loc: Location) -> Self {
        let mut array_expr = Self {
            keyword,
            elements,
            loc,
            force_multi_line: false,
            head_comment: None,
        };

        // Check if the rendered length exceeds the max line length
        // If so, force multi-line rendering
        if !array_expr.elements.is_empty() {
            let total_len = array_expr.last_line_len_from_left(0);
            if is_line_overflow(total_len) {
                array_expr.force_multi_line = true;
            }
        }

        array_expr
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// Set bind parameter comment before ARRAY keyword
    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment { text, mut loc } = comment;

        let text = trim_bind_param(text);

        self.head_comment = Some(text);
        loc.append(self.loc.clone());
        self.loc = loc;
    }

    /// Returns whether this array expression should be rendered as multi-line
    pub(crate) fn is_multi_line(&self) -> bool {
        self.force_multi_line
            || self
                .elements
                .iter()
                .any(|e| e.is_multi_line() || e.has_trailing_comment())
    }

    /// Returns the length of the last line when rendered
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        if self.is_multi_line() {
            // Multi-line ends with just "]"
            "]".len()
        } else {
            // keyword + "[" + elements + "]"
            let mut current_len = acc + self.keyword.len() + "[".len();
            if let Some(comment) = &self.head_comment {
                current_len += count_width(comment);
            }

            for (i, elem) in self.elements.iter().enumerate() {
                current_len = elem.last_line_len_from_left(current_len);
                if i != self.elements.len() - 1 {
                    current_len += ", ".len();
                }
            }

            current_len + "]".len()
        }
    }

    /// Render the array expression to a formatted string
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        if let Some(comment) = &self.head_comment {
            result.push_str(comment);
        }

        result.push_str(&self.keyword);

        if self.is_multi_line() {
            // Multi-line rendering
            result.push_str("[\n");

            // First line indent
            add_indent(&mut result, depth + 1);

            // Separator between elements: newline, comma, indent
            let mut separator = "\n".to_string();
            add_indent(&mut separator, depth);
            separator.push(',');
            add_space_by_range(&mut separator, 1, tab_size());

            // Compute alignment info
            let aligned_exprs = self.elements.iter().collect_vec();
            let align_info = AlignInfo::from(aligned_exprs);

            result.push_str(
                &self
                    .elements
                    .iter()
                    .map(|e| e.render_align(depth + 1, &align_info))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(&separator),
            );

            result.push('\n');
            add_indent(&mut result, depth);
            result.push(']');
        } else {
            // Single-line rendering
            result.push('[');
            result.push_str(
                &self
                    .elements
                    .iter()
                    .map(|e| e.render(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", "),
            );
            result.push(']');
        }

        Ok(result)
    }
}
