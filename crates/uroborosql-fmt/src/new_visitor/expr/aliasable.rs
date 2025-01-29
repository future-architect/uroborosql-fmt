use tree_sitter::TreeCursor;

use crate::{
    config::CONFIG,
    cst::*,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{create_alias, ensure_kind, error_annotation_from_cursor, Visitor, COMMENT},
};

/// 補完の種類
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ComplementKind {
    /// テーブル名
    TableName,
    /// カラム名
    ColumnName,
}

/// AS補完/除去、エイリアス補完に関する設定
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComplementConfig {
    /// 補完の種類
    kind: ComplementKind,
    /// kindに合わせてASキーワードを補完/除去するかどうか
    complement_or_remove_as: bool,
    /// kindに合わせてエイリアスを補完するかどうか
    complement_alias: bool,
}

impl Default for ComplementConfig {
    fn default() -> Self {
        ComplementConfig {
            // デフォルト値としてTableNameを設定しているが、デフォルトでは補完しないのでTableNameとColumnNameのどちらを設定していても変わらない
            kind: ComplementKind::TableName,
            complement_or_remove_as: false,
            complement_alias: false,
        }
    }
}

impl ComplementConfig {
    pub(crate) fn new(
        kind: ComplementKind,
        complement_or_remove_as: bool,
        complement_alias: bool,
    ) -> ComplementConfig {
        ComplementConfig {
            kind,
            complement_or_remove_as,
            complement_alias,
        }
    }

    /// 自身の設定と定義ファイルの設定を考慮してASを補完をすべきかどうか返す
    fn complement_as_keyword(&self) -> bool {
        self.complement_or_remove_as
            && CONFIG.read().unwrap().complement_column_as_keyword
            && self.kind == ComplementKind::ColumnName
    }

    /// 自身の設定と定義ファイルの設定を考慮してASを削除をすべきかどうか返す
    fn remove_as_keyword(&self) -> bool {
        self.complement_or_remove_as
            && CONFIG.read().unwrap().remove_table_as_keyword
            && self.kind == ComplementKind::TableName
    }

    /// 自身の設定と定義ファイルの設定を考慮してエイリアスを補完すべきかどうか返す
    pub(crate) fn complement_alias(&self) -> bool {
        self.complement_alias
            && CONFIG.read().unwrap().complement_alias
            && self.kind == ComplementKind::ColumnName
    }
}

impl Visitor {
    /// エイリアス可能な式
    /// 呼び出し後、cursorはaliasまたは式のノードを指している
    pub(crate) fn visit_aliasable_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        // エイリアス/AS補完に関する設定
        // Noneの場合は補完しない
        complement_config: Option<&ComplementConfig>,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // エイリアス可能な式の定義
        //    _aliasable_expression =
        //        alias | _expression

        //    alias =
        //        _expression
        //        ["AS"]
        //        identifier

        // 設定ファイルがNoneの場合はデフォルト値 (エイリアス/AS補完を共に行わない)
        let complement_config = complement_config.cloned().unwrap_or_default();

        let comment = if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        match cursor.node().kind() {
            "alias" => {
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let mut lhs_expr = self.visit_expr(cursor, src)?;
                if let Some(comment) = comment {
                    if comment.loc().is_next_to(&lhs_expr.loc()) {
                        lhs_expr.set_head_comment(comment);
                    } else {
                        // エイリアス式の直前のコメントは、バインドパラメータしか考慮していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_aliasable_expr(): unexpected comment\n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }

                let mut aligned = AlignedExpr::new(lhs_expr);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() {
                    // cursor -> trailing_comment | "AS"?

                    if cursor.node().kind() == COMMENT {
                        // ASの直前にcommentがある場合
                        let comment = Comment::new(cursor.node(), src);

                        if comment.is_block_comment() || !comment.loc().is_same_line(&aligned.loc())
                        {
                            // 行末以外のコメント(次以降の行のコメント)は未定義
                            // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                            // エイリアスがない場合は、コメントノードがここに現れない
                            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                "visit_aliasable_expr(): unexpected syntax\nnode_kind: {}\n{}",
                                cursor.node().kind(),
                                error_annotation_from_cursor(cursor, src)
                            )));
                        } else {
                            // 行末コメント
                            aligned.set_lhs_trailing_comment(comment)?;
                        }
                        cursor.goto_next_sibling();
                    }

                    let as_keyword = if cursor.node().kind() == "AS" {
                        let keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        cursor.goto_next_sibling();

                        // ASキーワードが存在する場合
                        if complement_config.remove_as_keyword() {
                            None
                        } else {
                            Some(convert_keyword_case(keyword))
                        }
                    } else {
                        // ASキーワードが存在しない場合
                        if complement_config.complement_as_keyword() {
                            Some(convert_keyword_case("AS"))
                        } else {
                            None
                        }
                    };

                    //右辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    ensure_kind(cursor, "identifier", src)?;

                    let rhs_expr =
                        PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Expr);
                    aligned.add_rhs(as_keyword, Expr::Primary(Box::new(rhs_expr)));
                }

                // cursorをaliasに戻す
                cursor.goto_parent();

                Ok(aligned)
            }
            _ => {
                // _expression
                let mut expr = self.visit_expr(cursor, src)?;

                if let Some(comment) = comment {
                    expr.set_head_comment(comment);
                }

                let mut aligned = AlignedExpr::new(expr.clone());

                if complement_config.complement_alias() {
                    // エイリアス名を生成できた場合にエイリアス補完を行う
                    if let Some(alias_name) = create_alias(&expr) {
                        aligned.add_rhs(Some(convert_keyword_case("AS")), alias_name);
                    }
                }

                Ok(aligned)
            }
        }
    }
}
