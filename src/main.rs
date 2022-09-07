use std::fs::read_to_string;
use std::fs::File;
use std::io::Write;

fn main() {
    let msg = "arguments error";
    let input_file = std::env::args().nth(1).expect(msg);

    let output_file = std::env::args().nth(2);

    let src = read_to_string(&input_file).unwrap();

    uroborosql_fmt::print_cst(src.as_ref());

    let result = uroborosql_fmt::format_sql(src.as_ref());

    match output_file {
        Some(path) => {
            let mut file = File::create(path).unwrap();
            file.write_all(result.as_bytes()).unwrap();
        }
        None => println!("{}", result),
    }
}
