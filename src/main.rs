use std::fs::read_to_string;
use std::fs::File;
use std::io::Write;

use uroborosql_fmt::format_sql;

fn main() {
    let msg = "arguments error";
    let input_file = std::env::args().nth(1).expect(msg);

    let output_file = std::env::args().nth(2);

    let src = read_to_string(&input_file).unwrap();

    let result = match format_sql(src.as_ref()) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("{}", e);
            src
        }
    };

    match output_file {
        Some(path) => {
            let mut file = File::create(path).unwrap();
            file.write_all(result.as_bytes()).unwrap();
        }
        None => println!("{}", result),
    }
}
