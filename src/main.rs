use std::fs::read_to_string;

fn main() {
    let msg = "arguments error";
    let filename = std::env::args().nth(1).expect(msg);

    let src = read_to_string(&filename).unwrap();

    uroborosql_fmt::print_cst(src.as_ref());

    let result = uroborosql_fmt::format_sql(src.as_ref());
    println!("{}", result);
}
