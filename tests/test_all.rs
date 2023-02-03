use std::{
    fs::{create_dir, read_to_string, DirEntry, File},
    io::Write,
    path::{self, PathBuf},
};

// 並列実行するとグローバル変数の問題が発生するため並列実行しない
#[test]
fn test() {
    test_all_files();
    test_config_file();
}

/// srcをconfigの設定でフォーマットした結果をdst_dirに保存
fn run_with_config(dst_dir: &PathBuf, src: &PathBuf, config: Option<&PathBuf>) {
    // file名
    let file_name = src.file_name().unwrap().to_str().unwrap();
    // fileの内容
    let content = read_to_string(&src).unwrap();

    let config_path = config.and_then(|c| c.to_str());

    let result = match uroborosql_fmt::format_sql(&content, config_path) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("{}", e);
            content.clone()
        }
    };

    // 出力先ファイル
    let mut dst_file = File::create(dst_dir.join(file_name)).unwrap();

    // 出力
    dst_file.write_all(result.as_bytes()).unwrap();
}

/*
   `cargo test`で、testfiles/src/にあるファイルすべてをフォーマットする
   フォーマット結果は、testfiles/dst/ディレクトリの同名ファイルに書き込まれる。
   commitしてあるファイルと比較し、違っていたらバグの可能性がある。
*/
fn test_all_files() {
    // testの対象を格納するディレクトリ
    let test_dir = path::PathBuf::from("./testfiles/");
    let src_dir = test_dir.join("src");
    let dst_dir = test_dir.join("dst");

    // src_dirに含まれるすべてのファイル、ディレクトリ
    let entries = src_dir.read_dir().unwrap();

    // デフォルト値の設定でテスト
    for entry in entries {
        let src_path = entry.unwrap().path();

        // ファイルかどうかをチェック
        if !src_path.is_file() {
            continue;
        }

        run_with_config(&dst_dir, &src_path, None);
    }
}

/*
   `cargo test`で、testfiles/config_test/src/にあるファイルすべてをtestfiles/config_test/configs内の各設定でフォーマットする
   フォーマット結果は、testfiles/dst_configX/ディレクトリの同名ファイルに書き込まれる。
   commitしてあるファイルと比較し、違っていたらバグの可能性がある。
*/
fn test_config_file() {
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

    for entry in &config_entries {
        let src_path = entry.path();

        // ファイルかどうかをチェック
        if !src_path.is_file() {
            continue;
        }

        run_with_config(&dst_dir, &src_path, None);
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
                .split(".")
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

            run_with_config(&dst_dir, &src_path, Some(&config.path()));
        }
    }
}
