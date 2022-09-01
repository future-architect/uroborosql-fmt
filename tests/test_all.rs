extern crate uroborosql_fmt;

use std::{
    fs::{read_to_string, File},
    io::Write,
    path,
};

/*
   `cargo test`で、testfiles/src/にあるファイルすべてをフォーマットする
   フォーマット結果は、testfiles/dst/ディレクトリの同名ファイルに書き込まれる。
   commitしてあるファイルと比較し、違っていたらバグの可能性がある。
*/
#[test]
fn test_all_files() {
    // testの対象を格納するディレクトリ
    let src_dir = path::PathBuf::from("./testfiles/src/");

    // src_dirに含まれるすべてのファイル、ディレクトリ
    let entries = src_dir.read_dir().unwrap();
    for entry in entries {
        let src_path = entry.unwrap().path();

        // ファイルかどうかをチェック
        if src_path.is_file() {
            // file名
            let file_name = src_path.file_name().unwrap().to_str().unwrap();
            // fileの内容
            let content = read_to_string(&src_path).unwrap();

            // フォーマット結果
            let result = uroborosql_fmt::format_sql(content.as_str());

            // 出力先
            let dst_path = String::from("./testfiles/dst/") + file_name;
            let mut dst_file = File::create(dst_path).unwrap();

            // 出力
            dst_file.write_all(result.as_bytes()).unwrap();
        }
    }
}
