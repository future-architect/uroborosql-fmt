use itertools::Itertools;

use crate::error::UroboroSQLFmtError;

use super::{dag::Kind, tree::TreeNode};

/// 現在のネストのENDまで読む
fn read_before_end(src_lines: &[&str], cursor: &mut usize) -> Vec<String> {
    let mut res_lines = vec![];
    let mut nest_count = 0;

    let mut is_first_line = true;

    while *cursor < src_lines.len() {
        // 現在のcursor位置の行を取得
        let current_line = src_lines[*cursor];

        let kind = Kind::guess_from_str(current_line);

        match kind {
            Kind::If | Kind::Begin => {
                // 入れ子カウントをインクリメント
                // ただし、一行目の場合は入れ子とは関係のない/*IF*/、/*BEGIN*/なので無視する
                if !is_first_line {
                    nest_count += 1;
                }
            }
            Kind::End => {
                if nest_count == 0 {
                    // 現在見ているネストは終了したのでbreak
                    break;
                } else {
                    // ネストを1つ抜ける
                    nest_count -= 1;
                }
            }
            _ => (),
        }

        res_lines.push(current_line.to_string());

        *cursor += 1;

        if is_first_line {
            is_first_line = false;
        }
    }

    res_lines
}

/// 複数SQLをマージする
fn merge_sql(sqls: Vec<String>) -> String {
    // もしマージ対象が1つの場合はそのまま返す
    if sqls.len() == 1 {
        return sqls[0].clone();
    }

    let mut res_lines = vec![];

    // 各SQLを行ごとに分割したもの
    let sql_lines = sqls.iter().map(|x| x.lines().collect_vec()).collect_vec();

    // 各SQLのカーソル
    let mut cursors = vec![0; sql_lines.len()];

    // cursorが行数をオーバーしている場合はtrueを返すクロージャ.
    let is_overflow = |cursors: &Vec<usize>| {
        for i in 0..sql_lines.len() {
            if cursors[i] >= sql_lines[i].len() {
                return true;
            }
        }
        false
    };

    while !is_overflow(&cursors) {
        // 各SQLのcusorが指す行を取得
        let current_lines = sql_lines
            .iter()
            .enumerate()
            .map(|(i, v)| v[cursors[i]])
            .collect_vec();

        if current_lines.iter().all_equal()
            || current_lines
                .iter()
                .all(|&x| matches!(Kind::guess_from_str(x), Kind::Plain))
        {
            // 全て一致している
            // または
            // 異なっているが全ての行がPLAINである (つまり、分岐に関係のない箇所である)
            // 場合
            // 1つ目のsqlの現在の行を描画

            // 演算子の立て揃えによって分岐に関係のない箇所で2つのSQLが異なる場合があるため、
            // 2つ目の条件の「異なっているが全ての行がPLAINである」によって分岐に関係のある箇所かどうか調べる必要がある
            //
            // 例えば以下のような場合を考える
            // ```
            //      1   =   1
            // /*IF hoge*/
            // AND  x   =   y
            // /*ELSE*/
            // AND  longlonglonglonglong    =   z
            // /*END*/
            // ```
            // 2つのSQLが生成され、1つ目のフォーマット結果は以下のようになる
            // ```
            //      1   =   1
            // /*IF hoge*/
            // AND  x   =   y
            // /*END*/
            // ```
            // 2つ目のフォーマット結果は「=」の立て揃え処理によって以下のようになる
            // ```
            //      1                       =   1
            // /*ELSE*/
            // AND  longlonglonglonglong    =   z
            // /*END*/
            // ```
            // これらは1行目で異なるが分岐に関係なくそのまま描画したいため、
            // 異なっていても全てPLAINである場合は1つ目のsqlの現在の行を描画する

            res_lines.push(current_lines[0].to_string());
        } else {
            // 分岐に関係のある箇所である場合
            // 各SQLを順にcursor位置から/*END*/まで読んで結果に格納
            for i in 0..sql_lines.len() {
                res_lines.append(&mut read_before_end(&sql_lines[i], &mut cursors[i]));
            }

            // 最終行まで出力した場合はbreak
            if is_overflow(&cursors) {
                break;
            }

            // /*END*/の描画
            //
            // sql_lines[i][cursors[i]]は全て同じ分岐の/*END*/を指している
            // サンプルとしてsql_lines[0][cursors[0]]の/*END*/を取得して結果に格納
            res_lines.push(sql_lines[0][cursors[0]].to_string());
        }

        // 全てのcursorを1つ進める
        cursors = cursors.iter().map(|x| x + 1).collect_vec();
    }

    // 各行を改行でjoin
    res_lines.join("\n")
}

/// treeの葉のSQLを全てマージ
pub(crate) fn merge_tree(tree: TreeNode) -> Result<String, UroboroSQLFmtError> {
    match tree {
        // 親の場合は再帰的に子をマージして返す
        TreeNode::Parent(nodes) => {
            let mut childs = vec![];

            // 子を再帰的にマージ
            for node in nodes {
                childs.push(merge_tree(node)?);
            }

            let merged = merge_sql(childs);

            Ok(merged)
        }
        // 葉の場合はそのまま返す
        TreeNode::Leaf(src) => Ok(src),
    }
}
