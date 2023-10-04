use once_cell::sync::Lazy;
use regex::Regex;

static IF_PATTERN: &str = r"/\*[ %]?(?i)(IF)[ ]+.*[ ]?\*/";
static ELIF_PATTERN: &str = r"(/\*[ ]?(ELIF)[ ]+.*[ ]?\*/)|(/\*%elseif[ ]+.*[ ]?\*/)";
static ELSE_PATTERN: &str = r"/\*[ %]?(?i)(ELSE)[ ]?\*/";
static END_PATTERN: &str = r"/\*[ %]?(?i)(END)[ ]?\*/";
static BEGIN_PATTERN: &str = r"/\*[ %]?(?i)(BEGIN)[ ]?\*/";

/// コンパイル済み正規表現を保持する構造体
pub(crate) struct Re {
    ///  `/*IF ..*/`にマッチするregex
    pub(crate) if_re: Regex,
    ///  `/*ELIF ..*/`にマッチするregex
    pub(crate) elif_re: Regex,
    /// `/*ELSE*/`にマッチするregex
    pub(crate) else_re: Regex,
    /// `/*END*/`にマッチするregex
    pub(crate) end_re: Regex,
    /// `/*BEGIN*/`にマッチするregex
    pub(crate) begin_re: Regex,
    ///  2way-sqlにおける分岐に関するキーワード(`/*IF ..*/`, `/*ELIF ..*/`,`/*ELSE*/`,`/*END*/`,`/*BEGIN*/`)にマッチするregex
    pub(crate) branching_keyword_re: Regex,
}

/// コンパイル済み正規表現を保持するグローバル変数
pub(crate) static RE: Lazy<Re> = Lazy::new(|| Re {
    if_re: Regex::new(IF_PATTERN).unwrap(),
    elif_re: Regex::new(ELIF_PATTERN).unwrap(),
    else_re: Regex::new(ELSE_PATTERN).unwrap(),
    end_re: Regex::new(END_PATTERN).unwrap(),
    begin_re: Regex::new(BEGIN_PATTERN).unwrap(),
    branching_keyword_re: Regex::new(
        format!(
            "({IF_PATTERN})|({ELIF_PATTERN})|({ELSE_PATTERN})|({END_PATTERN})|({BEGIN_PATTERN})"
        )
        .as_str(),
    )
    .unwrap(),
});
