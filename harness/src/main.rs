use std::env;
use std::io::{self, Write};
use stylebench_fixture::{generate, Config};

fn main() {
    let has = |f: &str| env::args().any(|a| a == f);
    let tiny = has("--tiny") || has("-t") || has("--tiny-sibling") || has("--tiny-structural");
    let sibling = has("--sibling") || has("--tiny-sibling");
    let structural = has("--structural") || has("--tiny-structural");
    let cfg = match (tiny, sibling, structural) {
        (true, true, _) => Config::tiny_sibling(),
        (true, _, true) => Config::tiny_structural(),
        (true, _, _) => Config::tiny(),
        (false, true, _) => Config::sibling_suite(),
        (false, _, true) => Config::structural_suite(),
        _ => Config::default_suite(),
    };
    let fix = generate(cfg);
    let mut out = io::stdout().lock();
    fix.write(&mut out).unwrap();
    let _ = out.flush();
}
