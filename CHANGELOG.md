# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.1](https://github.com/future-architect/uroborosql-fmt/releases/tag/v1.0.1) - 2026-05-25

### Added

- automate version management with release-plz ([#233](https://github.com/future-architect/uroborosql-fmt/pull/233))

### Fixed

- specify version requirement for postgresql-cst-parser git dependency ([#242](https://github.com/future-architect/uroborosql-fmt/pull/242))
- address clippy::collapsible_match warning introduced by Rust 1.95 ([#241](https://github.com/future-architect/uroborosql-fmt/pull/241))

### Other

- #115 fix: support CRLF line endings ([#232](https://github.com/future-architect/uroborosql-fmt/pull/232))
- #196 feat: support LATERAL JOIN with subquery ([#229](https://github.com/future-architect/uroborosql-fmt/pull/229))
- #71,#226,#227 feat: support array, fix: incorrect line length calculation and horizontal offset propagation bug ([#228](https://github.com/future-architect/uroborosql-fmt/pull/228))
- Fix case of LIKE and ILIKE ([#220](https://github.com/future-architect/uroborosql-fmt/pull/220))
- bump up to 1.0.1 ([#214](https://github.com/future-architect/uroborosql-fmt/pull/214))
- Support replacement parameter immediately after JOIN keyword ([#210](https://github.com/future-architect/uroborosql-fmt/pull/210))
- Support ILIKE ([#212](https://github.com/future-architect/uroborosql-fmt/pull/212))
- Preserve case of comments in WHERE clauses ([#208](https://github.com/future-architect/uroborosql-fmt/pull/208))
- Support order_by collate ([#206](https://github.com/future-architect/uroborosql-fmt/pull/206))
- パーサ置き換え（現行パーサと共存版） ([#116](https://github.com/future-architect/uroborosql-fmt/pull/116))
- clippy を適用 ([#157](https://github.com/future-architect/uroborosql-fmt/pull/157))
- Fix negative number alignment ([#142](https://github.com/future-architect/uroborosql-fmt/pull/142))
- Support indentation with spaces ([#111](https://github.com/future-architect/uroborosql-fmt/pull/111))
- Fix invalid testcases ([#110](https://github.com/future-architect/uroborosql-fmt/pull/110))
- Fix typo ([#102](https://github.com/future-architect/uroborosql-fmt/pull/102))
- Support comments before select_statement in insert_statement ([#101](https://github.com/future-architect/uroborosql-fmt/pull/101))
- Support comments after `SET` keyword ([#99](https://github.com/future-architect/uroborosql-fmt/pull/99))
- support comments after opening parenthesis in `IN` expression ([#98](https://github.com/future-architect/uroborosql-fmt/pull/98))
- Handle block comments after recursive keyword ([#97](https://github.com/future-architect/uroborosql-fmt/pull/97))
- Support comments after join keyword ([#96](https://github.com/future-architect/uroborosql-fmt/pull/96))
- Handle comments after select_statement at the end of insert_statement ([#93](https://github.com/future-architect/uroborosql-fmt/pull/93))
- Fix block comment alignment ([#92](https://github.com/future-architect/uroborosql-fmt/pull/92))
- Fix the vertical alignment of 2WaySQL ([#89](https://github.com/future-architect/uroborosql-fmt/pull/89))
- Supprt filter clause ([#85](https://github.com/future-architect/uroborosql-fmt/pull/85))
- Fix swapping positions of comma and comment ([#84](https://github.com/future-architect/uroborosql-fmt/pull/84))
- Fix consecutive comments at the end of with_clause parentheses ([#83](https://github.com/future-architect/uroborosql-fmt/pull/83))
- Support arithmetic expressions in limit clause ([#82](https://github.com/future-architect/uroborosql-fmt/pull/82))
- Support Unary operators ([#81](https://github.com/future-architect/uroborosql-fmt/pull/81))
- Correction indentation when the HAVING clause contains AND/OR ([#50](https://github.com/future-architect/uroborosql-fmt/pull/50))
- #46 の対応漏れ（タブ表示幅が2byte設定の際に、インデントが正しくない不具合を修正 ） ([#47](https://github.com/future-architect/uroborosql-fmt/pull/47))
- タブ表示幅が2byte設定の際に、インデントが正しくない不具合を修正 ([#46](https://github.com/future-architect/uroborosql-fmt/pull/46))
- Add an api that gives higher priority options ([#34](https://github.com/future-architect/uroborosql-fmt/pull/34))
- Support bind parameter in first colomn of select clause ([#38](https://github.com/future-architect/uroborosql-fmt/pull/38))
- Support using clause in DELETE statement ([#36](https://github.com/future-architect/uroborosql-fmt/pull/36))
- Improvement error message ([#33](https://github.com/future-architect/uroborosql-fmt/pull/33))
- Support join clause in UPDATE statement ([#30](https://github.com/future-architect/uroborosql-fmt/pull/30))
- Update version of tree-sitter-sql ([#20](https://github.com/future-architect/uroborosql-fmt/pull/20))
- Add option to convert "<>" to "!=" ([#18](https://github.com/future-architect/uroborosql-fmt/pull/18))
- Add workflow that tests and checks ([#14](https://github.com/future-architect/uroborosql-fmt/pull/14))
- Change default case format settings to lower ([#16](https://github.com/future-architect/uroborosql-fmt/pull/16))
- Support for ALL/DISTINCT/ORDER BY in aggregate functions ([#12](https://github.com/future-architect/uroborosql-fmt/pull/12))
- INSERT-SELECTのSELECTが括弧で囲まれている場合に対応 ([#2](https://github.com/future-architect/uroborosql-fmt/pull/2))
- tree-sitter-sql の参照先を github のものに変更
- Resolve "~enhancement SELECT DISTINCT [ ON ( expression [, ...] ) ]"
- SELECT FOR UPDATEに対応
- Resolve "~add postgresqlの `::` を使ったキャストをcast関数に書き換え"
- Resolve "ドキュメントの作成"
- #122 ~enhancement LIMIT句のバインドパラメータ対応
- Resolve "クレートを分割する"
