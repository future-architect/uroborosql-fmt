use crate::{cst::Location, error::UroboroSQLFmtError};

/// アスタリスクを表す。
/// テーブル名を含む場合もある。 (例: tab.*)
#[derive(Debug, Clone)]
pub(crate) struct AsteriskExpr {
    content: String,
    loc: Location,
}

impl AsteriskExpr {
    pub(crate) fn new(content: impl Into<String>, loc: Location) -> AsteriskExpr {
        let content = content.into();
        AsteriskExpr { content, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_len(&self) -> usize {
        self.content.len()
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        Ok(self.content.clone())
    }
}
