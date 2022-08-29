use std::{
    fs::File,
    io::Read,
};

fn main() {
    let mut f = File::open("./examples/simple.sql").unwrap();
    let mut src = String::new();
    f.read_to_string(&mut src).unwrap();

    uroborosql_fmt::print_cst(src.as_ref());

    let result = uroborosql_fmt::format_sql(src.as_ref());
    println!("{}", result);
}
