use std::env;
use std::io::{self, Write};
use stylebench_fixture::{generate, Config};

fn main() {
    let has = |f: &str| env::args().any(|a| a == f);
    let tiny = has("--tiny")
        || has("-t")
        || has("--tiny-sibling")
        || has("--tiny-structural")
        || has("--tiny-nth")
        || has("--tiny-before-after")
        || has("--tiny-media");
    let sibling = has("--sibling") || has("--tiny-sibling");
    let structural = has("--structural") || has("--tiny-structural");
    let nth = has("--nth") || has("--tiny-nth");
    let before_after = has("--before-after") || has("--tiny-before-after");
    let media = has("--media") || has("--tiny-media");
    let mut cfg = match (tiny, sibling, structural, nth, before_after, media) {
        (true, true, ..) => Config::tiny_sibling(),
        (true, _, true, ..) => Config::tiny_structural(),
        (true, _, _, true, ..) => Config::tiny_nth(),
        (true, _, _, _, true, _) => Config::tiny_before_after(),
        (true, _, _, _, _, true) => Config::tiny_media(),
        (true, ..) => Config::tiny(),
        (false, true, ..) => Config::sibling_suite(),
        (false, _, true, ..) => Config::structural_suite(),
        (false, _, _, true, ..) => Config::nth_suite(),
        (false, _, _, _, true, _) => Config::before_after_suite(),
        (false, _, _, _, _, true) => Config::media_suite(),
        _ => Config::default_suite(),
    };
    // --final-width=N: one extra resize after the StyleBench steps, so a
    // resize race can be checked at a width other than 800.
    if let Some(w) = env::args().find_map(|a| a.strip_prefix("--final-width=").map(str::to_string)) {
        cfg.resize_final = w.parse().expect("--final-width=N");
    }
    let fix = generate(cfg);
    let mut out = io::stdout().lock();
    fix.write(&mut out).unwrap();
    let _ = out.flush();
}
