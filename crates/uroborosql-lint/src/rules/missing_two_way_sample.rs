use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

/// Detects 2WaySQL bind parameter without sample values (e.g. `/*param*/`).
/// Rule source: https://future-architect.github.io/uroborosql-doc/background/#バインドパラメータ
pub struct MissingTwoWaySample;

impl Rule for MissingTwoWaySample {
    fn name(&self) -> &'static str {
        "missing-two-way-sample"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::C_COMMENT]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        assert_eq!(node.kind(), SyntaxKind::C_COMMENT);

        let comment = node;

        // 置換文字列はサンプル値がないケースもあるため除外
        if matches!(comment.text().chars().nth(2).unwrap(), '#' | '$') {
            return;
        }

        let Some(next_token) = comment.next_token() else {
            return;
        };

        // サンプル値抜けと判定する条件：
        // - next_token のテキストが空文字（パーサのエラー回復によって挿入されたトークン）
        // - next_token がコメント（元のノード）に隣接している
        if !next_token.text().is_empty() || !next_token.range().is_adjacent(&comment.range()) {
            return;
        }

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "sample value for bind parameter is missing.",
            &next_token.range(),
        );
        ctx.report(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, SqlSpan};

    fn run(sql: &str) -> Vec<Diagnostic> {
        run_with_rules(sql, vec![Box::new(MissingTwoWaySample)])
    }

    #[test]
    fn allows_numeric_sample() {
        let sql = "SELECT /*param1*/1;";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_empty_literal_sample() {
        let sql = "SELECT /*param1*/'';";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn detects_missing_sample() {
        let sql = "SELECT /*name*/ as name from t;";
        let diagnostics = run(sql);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "missing-two-way-sample")
            .expect("should detect missing sample");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "");
    }

    #[test]
    fn detects_missing_sample_from_lists() {
        let sql = "SELECT /*id*/1 as id,  /*name*/ as name from t;";
        let diagnostics = run(sql);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "missing-two-way-sample")
            .expect("should detect missing sample");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "");
    }

    #[test]
    fn ignores_control_comment() {
        let sql = r#"SELECT 1 from t 
        where id = 1
         /*IF cond*/
         OR id = 2
         /*END*/;"#;

        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_placeholder_without_sample_value() {
        let sql = "SELECT * FROM /*#table*/, /*$table*/ ;";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_normal_comments() {
        let sql = "SELECT /* comment 0 */ * /* comment 1 */ /* comment 2 */ FROM /* comment 3 */ t /* comment 4 */ ;";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }
}
