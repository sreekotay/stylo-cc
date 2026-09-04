use std::env;
use std::io::{self, Write};
use stylebench_fixture::{generate, Config};

fn main() {
    let tiny = env::args().any(|a| a == "--tiny" || a == "-t");
    let sibling = env::args().any(|a| a == "--sibling" || a == "--tiny-sibling");
    let cfg = if tiny || env::args().any(|a| a == "--tiny-sibling") {
        if sibling {
            Config::tiny_sibling()
        } else {
            Config::tiny()
        }
    } else if sibling {
        Config::sibling_suite()
    } else {
        Config::default_suite()
    };
    let fix = generate(cfg);
    let mut out = io::stdout().lock();
    fix.write(&mut out).unwrap();
    let _ = out.flush();
}
