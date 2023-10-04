use std::fs::read_to_string;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use uroborosql_fmt::format_sql;

fn main() {
    let msg = "arguments error";
    let input_file = std::env::args().nth(1).expect(msg);

    let output_file = std::env::args().nth(2);

    let src = read_to_string(input_file).unwrap();

    let config_path = match Path::is_file(Path::new("./uroborosqlfmt-config.json")) {
        true => Some("./uroborosqlfmt-config.json"),
        false => {
            eprintln!("hint: Create the file 'uroborosqlfmt-config.json' if you want to customize the configuration");
            None
        }
    };

    let result = match format_sql(src.as_ref(), config_path) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("{e}");
            src
        }
    };

    match output_file {
        Some(path) => {
            let mut file = File::create(path).unwrap();
            file.write_all(result.as_bytes()).unwrap();
        }
        None => println!("{result}"),
    }
}
