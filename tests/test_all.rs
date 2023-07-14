use std::{
    collections::HashMap,
    fs::{create_dir, create_dir_all, read_to_string, remove_dir_all, DirEntry, File},
    io::Write,
    panic,
    path::{self, Path, PathBuf},
};

use uroborosql_fmt::UroboroSQLFmtError;

// 並列実行するとグローバル変数の問題が発生するため並列実行しない
#[test]
fn test() {
    let result_all_files = test_all_files();
    let result_config_file = test_config_file();
    assert!(result_all_files);
    assert!(result_config_file);
}

/// srcをconfigの設定でフォーマットした結果をdst_dirに保存
fn run_with_config(
    dst_dir: &Path,
    src: &PathBuf,
    config: Option<&PathBuf>,
    failure_results: &mut HashMap<String, String>,
) {
    // file名
    let file_name = src.file_name().unwrap().to_str().unwrap();
    // fileの内容
    let content = read_to_string(src).unwrap();

    let config_path = config.and_then(|c| c.to_str());

    let result = match uroborosql_fmt::format_sql(&content, config_path) {
        Ok(format_result) => format_result,
        Err(UroboroSQLFmtError::Validation {
            format_result,
            error_msg,
        }) => {
            // assertion errorが生じた際は、Ok((フォーマット結果, エラーメッセージ))が返される
            failure_results.insert(src.to_str().unwrap().to_string(), error_msg);
            format_result
        }
        Err(e) => {
            failure_results.insert(src.to_str().unwrap().to_string(), e.to_string());
            content.clone()
        }
    };

    // 出力先ファイル
    let mut dst_file = File::create(dst_dir.join(file_name)).unwrap();

    // 出力
    dst_file.write_all(result.as_bytes()).unwrap();
}

/// `cargo test`で、testfiles/src/にあるファイルすべてをフォーマットする
/// フォーマット結果は、testfiles/dst/ディレクトリの同名ファイルに書き込まれる。
/// commitしてあるファイルと比較し、違っていたらバグの可能性がある。
fn test_all_files() -> bool {
    // testの対象を格納するディレクトリ
    let test_dir = path::PathBuf::from("./testfiles/");
    let src_dir = test_dir.join("src");
    let dst_dir = test_dir.join("dst");

    // 最初に ./testfiles/dir/を削除しておく
    remove_dir_all(&dst_dir).unwrap_or_else(|_| eprintln!("./testfiles/dst/ does not exists"));

    create_dir_all(&dst_dir).expect("Directory ./testfiles.dst cannot be created.");

    let entries = src_dir.read_dir().unwrap();

    let mut failure_results = HashMap::new();

    // デフォルト値の設定でテスト
    entries.for_each(|e| test_entry_with_config(e.unwrap(), "", None, &mut failure_results));

    if !failure_results.is_empty() {
        eprintln!("-- test_all_files out --");
        eprintln!("failed files ...");
        failure_results.iter().for_each(|(path, error_msg)| {
            eprintln!("{}: {}", path, error_msg);
        });
        eprintln!("{} files failed", failure_results.len());
        return false;
    }
    true
}

/// `cargo test`で、testfiles/config_test/src/にあるファイルすべてをtestfiles/config_test/configs内の各設定でフォーマットする
/// フォーマット結果は、testfiles/dst_configX/ディレクトリの同名ファイルに書き込まれる。
/// commitしてあるファイルと比較し、違っていたらバグの可能性がある。
fn test_config_file() -> bool {
    let config_test_dir = path::PathBuf::from("./testfiles/config_test/");
    let configs_dir = config_test_dir.join("configs");
    let configs: Vec<DirEntry> = configs_dir
        .read_dir()
        .unwrap()
        .map(|test| test.unwrap())
        .collect();

    // testの対象を格納するディレクトリ
    let config_src_dir = config_test_dir.join("src");

    // config_src_dirに含まれるすべてのファイル、ディレクトリ
    let config_entries: Vec<DirEntry> = config_src_dir
        .read_dir()
        .unwrap()
        .map(|test| test.unwrap())
        .collect();

    // デフォルト
    let dst_dir = config_test_dir.join("dst_default");
    // 出力先ディレクトリの作成
    let _ = create_dir(&dst_dir);

    let mut failure_results = HashMap::new();

    for entry in &config_entries {
        let src_path = entry.path();

        // ファイルかどうかをチェック
        if !src_path.is_file() {
            continue;
        }

        run_with_config(&dst_dir, &src_path, None, &mut failure_results);
    }

    // configsに含まれる設定
    for config in &configs {
        // 出力先ディレクトリ
        let dst_dir = config_test_dir.join(format!(
            "dst_{}",
            config
                .file_name()
                .to_str()
                .unwrap() // file名 (例: config1.json)
                .split('.')
                .next()
                .unwrap() // 拡張子を外したファイル名 (例: config1)
        ));

        // 出力先ディレクトリの作成
        let _ = create_dir(&dst_dir);

        for entry in &config_entries {
            let src_path = entry.path();

            // ファイルかどうかをチェック
            if !src_path.is_file() {
                continue;
            }

            run_with_config(
                &dst_dir,
                &src_path,
                Some(&config.path()),
                &mut failure_results,
            );
        }
    }

    if !failure_results.is_empty() {
        eprintln!("-- test_config_file out --");
        eprintln!("failed files ...");
        failure_results.iter().for_each(|(path, error_msg)| {
            eprintln!("{}: {}", path, error_msg);
        });
        eprintln!("{} files failed", failure_results.len());
        return false;
    }
    true
}

fn test_entry_with_config(
    entry: DirEntry,
    rel_path: &str,
    config: Option<&PathBuf>,
    failure_results: &mut HashMap<String, String>,
) {
    let src_path = entry.path();
    if src_path.is_dir() {
        let dir_name = src_path.file_name().unwrap().to_str().unwrap();
        let directory_path = ("./testfiles/dst/".to_owned() + rel_path) + dir_name;

        // dstディレクトリに、対応するディレクトリを生成
        if let Err(e) = create_dir_all(path::Path::new(&directory_path)) {
            panic!("create_dir: {:?}", e)
        }

        let entries = src_path.read_dir().unwrap();
        let rel_path = rel_path.to_owned() + dir_name + "/";

        entries
            .for_each(|e| test_entry_with_config(e.unwrap(), &rel_path, config, failure_results));
    } else if src_path.is_file() {
        // ファイルの拡張子が.sql出ない場合は飛ばす
        let ext = src_path.extension().unwrap();
        if ext != "sql" {
            return;
        }

        // 出力先
        let dst_dir = path::PathBuf::from("./testfiles/dst/");
        let dst_dir = dst_dir.join(rel_path);

        // フォーマットをデフォルト設定で実行
        run_with_config(&dst_dir, &src_path, config, failure_results);
    }
}
