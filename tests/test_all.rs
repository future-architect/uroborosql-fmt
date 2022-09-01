extern crate uroborosql_fmt;

use std::{
    fs::{read_to_string, File},
    io::Write,
    path,
};

#[test]
fn test_all_files() {
    let src_dir = path::PathBuf::from("./testfiles/src/");

    let files = src_dir.read_dir().unwrap();
    for dir_entry in files {
        let src_path = dir_entry.unwrap().path();

        if src_path.is_file() {
            let file_name = src_path.file_name().unwrap().to_str().unwrap();
            let src = read_to_string(&src_path).unwrap();

            let result = uroborosql_fmt::format_sql(src.as_str());

            let dst_path = String::from("./testfiles/dst/") + file_name;
            let mut dst_file = File::create(dst_path).unwrap();
            dst_file.write_all(result.as_bytes()).unwrap();
        }
    }
}
