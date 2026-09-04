use std::env;
use std::io::{self, Write};
use stylebench_fixture::{generate, Config};

fn main() {
    let has = |f: &str| env::args().any(|a| a == f);
    let tiny = has("--tiny")
        || has("-t")
        || has("--tiny-sibling")
        || has("--tiny-structural")
        || has("--tiny-nth");
    let sibling = has("--sibling") || has("--tiny-sibling");
    let structural = has("--structural") || has("--tiny-structural");
    let nth = has("--nth") || has("--tiny-nth");
    let cfg = match (tiny, sibling, structural, nth) {
        (true, true, _, _) => Config::tiny_sibling(),
        (true, _, true, _) => Config::tiny_structural(),
        (true, _, _, true) => Config::tiny_nth(),
        (true, _, _, _) => Config::tiny(),
        (false, true, _, _) => Config::sibling_suite(),
        (false, _, true, _) => Config::structural_suite(),
        (false, _, _, true) => Config::nth_suite(),
        _ => Config::default_suite(),
    };
    let fix = generate(cfg);
    let mut out = io::stdout().lock();
    fix.write(&mut out).unwrap();
    let _ = out.flush();
}
