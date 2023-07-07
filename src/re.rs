use once_cell::sync::Lazy;
use regex::Regex;

/// コンパイル済み正規表現を保持する構造体
pub(crate) struct Re {
    ///  `/*IF ..*/`にマッチするregex
    pub(crate) if_re: Regex,
    ///  `/*ELIF ..*/`にマッチするregex
    pub(crate) elif_re: Regex,
    ///  2way-sqlにおける分岐に関するキーワード(`/*IF ..*/`, `/*ELIF ..*/`,`/*ELSE*/`,`/*END*/`,`/*BEGIN*/`)にマッチするregex
    pub(crate) branching_keyword_re: Regex,
}

/// コンパイル済み正規表現を保持するグローバル変数
pub(crate) static RE: Lazy<Re> = Lazy::new(|| Re {
    if_re: Regex::new(r"(/\*IF).*(\*/)").unwrap(),
    elif_re: Regex::new(r"(/\*ELIF).*(\*/)").unwrap(),
    branching_keyword_re: Regex::new(r"(/\*IF).*(\*/)|(/\*ELIF).*(\*/)|/\*(ELSE|END|BEGIN)\*/")
        .unwrap(),
});
