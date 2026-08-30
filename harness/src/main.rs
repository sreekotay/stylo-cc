use std::env;
use std::io::{self, Write};
use stylebench_fixture::{generate, Config};

fn main() {
    let tiny = env::args().any(|a| a == "--tiny" || a == "-t");
    let cfg = if tiny {
        Config::tiny()
    } else {
        Config::default_suite()
    };
    let fix = generate(cfg);
    let mut out = io::stdout().lock();
    fix.write(&mut out).unwrap();
    let _ = out.flush();
}
