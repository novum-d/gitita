use std::env;
use std::process;

fn main() {
    if let Err(error) = gitita::run(env::args().skip(1)) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
