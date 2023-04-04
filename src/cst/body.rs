use itertools::{repeat_n, Itertools};

use super::{AlignedExpr, BooleanExpr, ColumnList, Comment, Expr, Location, UroboroSQLFmtError};

/// 句の本体を表す
#[derive(Debug, Clone)]
pub(crate) enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
    Insert(Box<InsertBody>),
    /// Clause と Expr を単一行で描画する際の Body
    SingleLine(Box<SingleLine>),
}

impl Body {
    /// 本体の要素が空である場合 None を返す
    pub(crate) fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
            Body::Insert(insert) => Some(insert.loc()),
            Body::SingleLine(expr_body) => Some(expr_body.loc()),
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(depth),
            Body::BooleanExpr(bool_expr) => bool_expr.render(depth),
            Body::Insert(insert) => insert.render(depth),
            Body::SingleLine(single_line) => single_line.render(depth),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment)?,
            Body::BooleanExpr(bool_expr) => bool_expr.add_comment_to_child(comment)?,
            Body::Insert(insert) => insert.add_comment_to_child(comment)?,
            Body::SingleLine(single_line) => single_line.add_comment_to_child(comment)?,
        }

        Ok(())
    }

    /// bodyの要素が空であるかどうかを返す
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.is_empty(),
            Body::BooleanExpr(bool_expr) => bool_expr.is_empty(),
            Body::Insert(_) => false, // InsertBodyには必ずtable_nameが含まれる
            Body::SingleLine(_) => false,
        }
    }

    /// 一つのExprからなるBodyを生成し返す
    pub(crate) fn with_expr(expr: Expr) -> Body {
        if expr.is_body() {
            // Bodyである場合はそのまま返せばよい
            if let Expr::Boolean(boolean) = expr {
                Body::BooleanExpr(*boolean)
            } else {
                // error
                unimplemented!()
            }
        } else {
            // Bodyでない場合、SeparatedLinesにして返す
            let mut sep_lines = SeparatedLines::new("", false);
            sep_lines.add_expr(expr.to_aligned());
            Body::SepLines(sep_lines)
        }
    }

    /// 単一行の Clause の Body となる SingleLineを生成する
    pub(crate) fn to_single_line(expr: Expr) -> Body {
        Body::SingleLine(Box::new(SingleLine::new(expr)))
    }

    /// Body に含まれる最初の式にバインドパラメータをセットすることを試みる。
    /// セットできた場合は true を返し、できなかった場合は false を返す。
    pub(crate) fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.try_set_head_comment(comment),
            Body::BooleanExpr(boolean) => boolean.try_set_head_comment(comment),
            Body::Insert(_) => false,
            Body::SingleLine(single_line) => single_line.try_set_head_comment(comment),
        }
    }
}

/// 句の本体にあたる部分である、あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub(crate) struct SeparatedLines {
    /// セパレータ(e.g., ',', AND)
    separator: String,
    /// 各行の情報。式と直後に来るコメントのペアのベクトルとして保持する
    contents: Vec<(AlignedExpr, Vec<Comment>)>,
    loc: Option<Location>,
    /// 縦ぞろえの対象となる演算子があるかどうか
    has_op: bool,
    /// render時に AS を省略するかどうか
    is_from_body: bool,
}

impl SeparatedLines {
    pub(crate) fn new(sep: impl Into<String>, is_omit_op: bool) -> SeparatedLines {
        let separator = sep.into();
        SeparatedLines {
            separator,
            contents: vec![] as Vec<(AlignedExpr, Vec<Comment>)>,
            loc: None,
            has_op: false,
            is_from_body: is_omit_op,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // 式を追加する
    pub(crate) fn add_expr(&mut self, aligned: AlignedExpr) {
        // 演算子があるかどうかをチェック
        if aligned.has_rhs() {
            self.has_op = true;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        self.contents.push((aligned, vec![]));
    }

    /// 最後の式にコメントを追加する
    /// 最後の式と同じ行である場合は行末コメントとして追加し、そうでない場合は式の下のコメントとして追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        let comment_loc = comment.loc();

        if comment.is_multi_line_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            // 最後の要素にコメントを追加
            self.contents.last_mut().unwrap().1.push(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.contents
                .last_mut()
                .unwrap()
                .0
                .set_trailing_comment(comment)?;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(comment_loc),
            None => self.loc = Some(comment_loc),
        };

        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        if let Some((first_aligned, _)) = self.contents.first_mut() {
            if comment.loc().is_next_to(&first_aligned.loc()) {
                first_aligned.set_head_comment(comment);
                return true;
            }
        }
        false
    }

    /// AS句で揃えたものを返す
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        if depth < 1 {
            // ','の後にタブ文字を挿入するので、インデントの深さ(depth)は1以上でなければならない。
            return Err(UroboroSQLFmtError::RenderingError(
                "SeparatedLines::render(): The depth must be bigger than 0".to_owned(),
            ));
        }

        let mut result = String::new();

        // 演算子自体の長さ
        let align_info = self.contents.iter().map(|(a, _)| a).collect_vec().into();
        let mut is_first_line = true;

        for (aligned, comments) in &self.contents {
            result.extend(repeat_n('\t', depth - 1));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push_str(&self.separator);
            }
            result.push('\t');

            // alignedに演算子までの最長の長さを与えてフォーマット済みの文字列をもらう
            let formatted = aligned.render_align(depth, &align_info, self.is_from_body)?;
            result.push_str(&formatted);
            result.push('\n');

            // commentsのrender
            for comment in comments {
                result.push_str(&comment.render(depth - 1)?);
                result.push('\n');
            }
        }

        Ok(result)
    }
}

/// INSERT文の本体。
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct InsertBody {
    loc: Location,
    table_name: AlignedExpr,
    columns: Option<SeparatedLines>,
    values_kw: Option<String>,
    values_rows: Vec<ColumnList>,
}

impl InsertBody {
    pub(crate) fn new(loc: Location, table_name: AlignedExpr) -> InsertBody {
        InsertBody {
            loc,
            table_name,
            columns: None,
            values_kw: None,
            values_rows: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// カラム名をセットする
    pub(crate) fn set_column_name(&mut self, cols: SeparatedLines) {
        self.columns = Some(cols);
    }

    /// VALUES句をセットする
    pub(crate) fn set_values_clause(&mut self, kw: &str, body: Vec<ColumnList>) {
        self.values_kw = Some(kw.to_string());
        self.values_rows = body;
    }

    /// 子供にコメントを追加する
    ///
    /// 対応済み
    /// - テーブル名の行末コメント
    ///
    /// 未対応
    /// - VALUES句の直後に現れるコメント
    /// - VALUES句の本体に現れるコメント
    /// - カラム名の直後に現れるコメント
    /// - テーブル名の直後に現れるコメント
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // 下から順番に見ていく

        // table_nameの直後に現れる
        if comment.is_multi_line_comment() || !self.table_name.loc().is_same_line(&comment.loc()) {
            // 行末コメントではない場合は未対応
            unimplemented!()
        } else {
            // 行末コメントである場合、table_nameに追加する
            self.table_name.set_trailing_comment(comment)?;
        }

        Ok(())
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は INSERT が描画される行のインデントの深さ + 1 (つまり、テーブル名が描画される行の深さ)
        if depth < 1 {
            // インデントの深さ(depth)は1以上でなければならない。
            return Err(UroboroSQLFmtError::RenderingError(
                "InsertBody::render(): The depth must be bigger than 0".to_owned(),
            ));
        }

        let mut result = String::new();

        // テーブル名
        result.extend(repeat_n('\t', depth));
        result.push_str(&self.table_name.render(depth)?);
        result.push('\n');

        // カラム名
        if let Some(sep_lines) = &self.columns {
            result.extend(repeat_n('\t', depth - 1));
            result.push_str("(\n");
            result.push_str(&sep_lines.render(depth)?);
            result.push(')');
        }

        // VALUES句
        if let Some(kw) = &self.values_kw {
            result.push(' ');
            result.push_str(kw);

            // 要素が一つか二つ以上かでフォーマット方針が異なる
            let is_one_row = self.values_rows.len() == 1;

            if !is_one_row {
                result.push('\n');
                result.extend(repeat_n('\t', depth));
            } else {
                // "VALUES" と "(" の間の空白
                result.push(' ');
            }

            let mut separator = String::from('\n');
            separator.extend(repeat_n('\t', depth - 1));
            separator.push_str(",\t");

            result.push_str(
                &self
                    .values_rows
                    .iter()
                    .filter_map(|cols| cols.render(depth - 1).ok())
                    .join(&separator),
            );
            result.push('\n');
        } else {
            // VALUES句があるときは、改行を入れずに`VALUES`キーワードを出力している
            // そのため、VALUES句がない場合はここで改行
            result.push('\n');
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleLine {
    expr: AlignedExpr,
    loc: Location,
    comments: Vec<Comment>,
}

impl SingleLine {
    pub(crate) fn new(expr: Expr) -> SingleLine {
        let expr = expr.to_aligned();
        let loc = expr.loc();
        SingleLine {
            expr,
            loc,
            comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_multi_line_comment() || !self.loc.is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            self.comments.push(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.expr.set_trailing_comment(comment)?;
        }
        Ok(())
    }

    fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        if comment.loc().is_next_to(&self.expr.loc()) {
            self.expr.set_head_comment(comment);
            true
        } else {
            false
        }
    }

    /// 先頭にインデントを挿入せずに render する。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 式は一つのみであるため、縦ぞろえはしない
        result.push_str(&self.expr.render(depth)?);

        result.push('\n');
        if !self.comments.is_empty() {
            result.push_str(
                &self
                    .comments
                    .iter()
                    .map(|c| c.render(depth))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\n"),
            );
            result.push('\n');
        }

        Ok(result)
    }
}
