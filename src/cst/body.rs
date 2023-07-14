use itertools::{repeat_n, Itertools};

use crate::util::{convert_identifier_case, convert_keyword_case};

use super::{
    AlignedExpr, BooleanExpr, Clause, ColumnList, Comment, ConflictTargetColumnList, Expr,
    Location, SubExpr, UroboroSQLFmtError,
};

/// 句の本体を表す
#[derive(Debug, Clone)]
pub(crate) enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
    Insert(Box<InsertBody>),
    With(Box<WithBody>),
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
            Body::With(with) => with.loc(),
            Body::SingleLine(expr_body) => Some(expr_body.loc()),
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(depth),
            Body::BooleanExpr(bool_expr) => bool_expr.render(depth),
            Body::Insert(insert) => insert.render(depth),
            Body::With(with) => with.render(depth),
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
            Body::With(with) => with.add_comment_to_child(comment)?,
            Body::SingleLine(single_line) => single_line.add_comment_to_child(comment)?,
        }

        Ok(())
    }

    /// bodyの要素が空であるかどうかを返す
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.is_empty(),
            Body::BooleanExpr(bool_expr) => bool_expr.is_empty(),
            Body::With(_) => false, // WithBodyには必ずwith_contentsが含まれる
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
            Body::With(_) => false,
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

        if comment.is_block_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
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
            return Err(UroboroSQLFmtError::Rendering(
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

/// INSERT文のconflict_targetにおいてindexカラムを指定した場合
#[derive(Debug, Clone)]
pub(crate) struct SpecifyIndexColumn {
    index_expression: ConflictTargetColumnList,
    where_clause: Option<Clause>,
}

impl SpecifyIndexColumn {
    pub(crate) fn new(index_expression: ConflictTargetColumnList) -> SpecifyIndexColumn {
        SpecifyIndexColumn {
            index_expression,
            where_clause: None,
        }
    }

    /// where句の追加
    pub(crate) fn set_where_clause(&mut self, where_clause: Clause) {
        self.where_clause = Some(where_clause);
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.index_expression.render(depth)?);
        result.push('\n');

        if let Some(where_clause) = &self.where_clause {
            result.push_str(&where_clause.render(depth - 1)?);
        }

        Ok(result)
    }
}

/// INSERT文のconflict_targetにおけるON CONSTRAINT
#[derive(Debug, Clone)]
pub(crate) struct OnConstraint {
    /// (ON, CONSTRAINT)
    keyword: (String, String),
    constraint_name: String,
}

impl OnConstraint {
    pub(crate) fn new(keyword: (String, String), constraint_name: String) -> OnConstraint {
        OnConstraint {
            keyword,
            constraint_name,
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        // ON
        result.push_str(&convert_keyword_case(&self.keyword.0));
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // CONSTRAINT
        result.push_str(&convert_keyword_case(&self.keyword.1));
        result.push('\t');
        result.push_str(&self.constraint_name);
        result.push('\n');

        Ok(result)
    }
}

/// INSERT文におけるconflict_target
#[derive(Debug, Clone)]
pub(crate) enum ConflictTarget {
    SpecifyIndexColumn(SpecifyIndexColumn),
    OnConstraint(OnConstraint),
}

/// INSERT文のconflict_actionにおけるDO NOTHING
#[derive(Debug, Clone)]
pub(crate) struct DoNothing {
    /// (DO, NOTHING)
    keyword: (String, String),
}

impl DoNothing {
    pub(crate) fn new(keyword: (String, String)) -> DoNothing {
        DoNothing { keyword }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.extend(repeat_n('\t', depth - 1));
        // DO
        result.push_str(&convert_keyword_case(&self.keyword.0));
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // NOTHING
        result.push_str(&convert_keyword_case(&self.keyword.1));
        result.push('\n');

        Ok(result)
    }
}

/// INSERT文のconflict_actionにおけるDO UPDATE
#[derive(Debug, Clone)]
pub(crate) struct DoUpdate {
    /// (DO, UPDATE)
    keyword: (String, String),
    set_clause: Clause,
    where_clause: Option<Clause>,
}

impl DoUpdate {
    pub(crate) fn new(
        keyword: (String, String),
        set_clause: Clause,
        where_clause: Option<Clause>,
    ) -> DoUpdate {
        DoUpdate {
            keyword,
            set_clause,
            where_clause,
        }
    }
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.extend(repeat_n('\t', depth - 1));
        // DO
        result.push_str(&convert_keyword_case(&self.keyword.0));
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // UPDATE
        result.push_str(&convert_keyword_case(&self.keyword.1));
        result.push('\n');
        // SET句
        result.push_str(&self.set_clause.render(depth)?);
        // WHERE句
        if let Some(where_clause) = &self.where_clause {
            result.push_str(&where_clause.render(depth)?);
        }

        Ok(result)
    }
}

/// INSERT文におけるconflict_action
#[derive(Debug, Clone)]
pub(crate) enum ConflictAction {
    DoNothing(DoNothing),
    DoUpdate(DoUpdate),
}

/// INSERT文におけるON CONFLICT
#[derive(Debug, Clone)]
pub(crate) struct OnConflict {
    /// (ON CONFLICT)
    keyword: (String, String),
    conflict_target: Option<ConflictTarget>,
    conflict_action: ConflictAction,
}

impl OnConflict {
    pub(crate) fn new(
        keyword: (String, String),
        conflict_target: Option<ConflictTarget>,
        conflict_action: ConflictAction,
    ) -> OnConflict {
        OnConflict {
            keyword,
            conflict_target,
            conflict_action,
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.extend(repeat_n('\t', depth - 1));
        // ON
        result.push_str(&convert_keyword_case(&self.keyword.0));
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // CONFLICT
        result.push_str(&convert_keyword_case(&self.keyword.1));

        if let Some(conflict_target) = &self.conflict_target {
            match conflict_target {
                ConflictTarget::OnConstraint(on_constraint) => {
                    // ON CONSTRAINTの場合は改行して描画
                    result.push('\n');
                    result.push_str(&on_constraint.render(depth)?);
                }
                ConflictTarget::SpecifyIndexColumn(specify_index_column) => {
                    // INDEXカラム指定の場合は改行せずに描画
                    result.push('\t');
                    result.push_str(&specify_index_column.render(depth)?);
                }
            }
        } else {
            // conflict_targetがない場合は改行
            result.push('\n');
        }

        match &self.conflict_action {
            ConflictAction::DoNothing(do_nothing) => result.push_str(&do_nothing.render(depth)?),
            ConflictAction::DoUpdate(do_update) => result.push_str(&do_update.render(depth)?),
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
    on_conflict: Option<OnConflict>,
}

impl InsertBody {
    pub(crate) fn new(loc: Location, table_name: AlignedExpr) -> InsertBody {
        InsertBody {
            loc,
            table_name,
            columns: None,
            values_kw: None,
            values_rows: vec![],
            on_conflict: None,
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

    pub(crate) fn set_on_conflict(&mut self, on_conflict: OnConflict) {
        self.on_conflict = Some(on_conflict);
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
        if comment.is_block_comment() || !self.table_name.loc().is_same_line(&comment.loc()) {
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
            return Err(UroboroSQLFmtError::Rendering(
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
            result.extend(repeat_n('\t', depth - 1));
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

        if let Some(oc) = &self.on_conflict {
            result.push_str(&oc.render(depth)?);
        }

        Ok(result)
    }
}

/// WITH句における名前付きサブクエリ}
/// cte (Common Table Expressions)
#[derive(Debug, Clone)]
pub(crate) struct Cte {
    loc: Location,
    name: String,
    as_keyword: String,
    column_name: Option<ColumnList>,
    materialized_keyword: Option<String>,
    sub_expr: SubExpr,
    /// 行末コメント
    trailing_comment: Option<String>,
    /// テーブル名の直後に現れる行末コメント
    name_trailing_comment: Option<String>,
}

impl Cte {
    pub(crate) fn new(
        loc: Location,
        name: String,
        as_keyword: String,
        column_name: Option<ColumnList>,
        materialized_keyword: Option<String>,
        statement: SubExpr,
    ) -> Cte {
        Cte {
            loc,
            name,
            as_keyword,
            column_name,
            materialized_keyword,
            sub_expr: statement,
            trailing_comment: None,
            name_trailing_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// cteのtrailing_commentをセットする
    /// 複数行コメントを与えた場合エラーを返す
    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_trailing_comment:{:?} is not trailing comment!",
                comment
            )))
        } else {
            let Comment { text, loc } = comment;
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    /// テーブル名のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_name_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_name_trailing_comment:{:?} is not trailing comment!",
                comment
            )))
        } else {
            // 行コメント
            let Comment { text, loc } = comment;
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());
            self.name_trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&convert_identifier_case(&self.name));
        result.push('\t');

        // カラム名の指定がある場合
        if let Some(column_list) = &self.column_name {
            result.push_str(&column_list.render(depth)?);
            result.push('\t');
        }

        // テーブル名の直後のコメントがある場合
        if let Some(comment) = &self.name_trailing_comment {
            result.push_str(comment);
            result.push('\n');
            result.extend(repeat_n('\t', depth));
        }

        result.push_str(&convert_keyword_case(&self.as_keyword));
        result.push('\t');

        // MATERIALIZEDの指定がある場合
        if let Some(materialized) = &self.materialized_keyword {
            result.push_str(&convert_keyword_case(materialized));
            result.push('\t');
        }

        result.push_str(&self.sub_expr.render(depth)?);

        if let Some(comment) = &self.trailing_comment {
            result.push('\t');
            result.push_str(comment);
        }

        Ok(result)
    }
}

/// WITH句の本体。
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct WithBody {
    loc: Option<Location>,
    contents: Vec<(Cte, Vec<Comment>)>,
}

impl WithBody {
    pub(crate) fn new() -> WithBody {
        WithBody {
            loc: None,
            contents: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // cteを追加する
    pub(crate) fn add_cte(&mut self, cte: Cte) {
        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(cte.loc()),
            None => self.loc = Some(cte.loc()),
        };

        self.contents.push((cte, vec![]));
    }

    /// 最後のcteにコメントを追加する
    /// 最後のcteと同じ行である場合は行末コメントとして追加し、そうでない場合はcteの下のコメントとして追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        let comment_loc = comment.loc();

        if comment.is_block_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
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

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        let mut is_first_line = true;

        for (cte, comments) in &self.contents {
            result.extend(repeat_n('\t', depth - 1));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push(',')
            }
            result.push('\t');

            let formatted = cte.render(depth)?;
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
        if comment.is_block_comment() || !self.loc.is_same_line(&comment.loc()) {
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
