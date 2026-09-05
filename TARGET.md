# Target

Two standalone runners, one frozen [StyleBench](https://github.com/WebKit/WebKit/blob/main/PerformanceTests/StyleBench/resources/style-bench.js) workload, one gate: the same computed style for every element, byte for byte, before any time is read.

```
StyleBench workload generator / fixtures (harness/, same LCG and seeds as WebKit)
                 |
        -------------------
        |                 |
   Stylo runner        CC runner
   idiomatic Rust      idiomatic CC
        |                 |
   computed styles     computed styles
   + timings           + timings
```

No Firefox. No Gecko. No Servo patch. No wrapping Stylo’s Rayon walk.

## Competitor

**Stylo itself** (`style`: match, cascade, `RuleTree`, `recalc_style_at`, proto / COW, sharing LRU, snapshots + invalidation map). The `TElement` / `TNode` slab in `stylo-runner/` is our host — glue, like the CC fixture load. Do not score `host.rs` against the CC engine; do fix it when it hides work Stylo would do in a browser (it posts `RESTYLE_STYLE_ATTRIBUTE` on a `style=` change, as Gecko does).

## The CC runner

A styling engine — tree, match, cascade, computed values, inherit, invalidation — written the [CC way](https://github.com/sreekotay/concurrent-c/blob/main/docs/the-cc-way.md). Honor the contract, keep the CC shape: do what the engine does, not how Stylo does it.

- **Same result, full burden.** Every longhand Stylo exposes to content (189) is cascaded, inherited, resolved and printed, per element and per kept `::before` / `::after` box. The list is Stylo's own (`stylo-runner --longhands`); `scripts/gen-longhands.shcc` turns it into a table and a `@comptime` function (`engine/sty_emit.cch`) emits the property layer from that table at compile time. Adding a longhand is a table row, not code.
- **One family of matches.** Descendant / child / sibling combinators, structural and `:nth-*` pseudo-classes, `::before` / `::after`, `@media` are all compound facts in the same buckets and the same `match_from`. No property, selector, or suite is special-cased; no option is tuned per suite (workers size themselves).
- **Sequential first, parallel once the relations are nameable.** Match runs `@parallel for` over share canons in worker-sized arms; signature and invalidation passes run in parallel because they only read the tree; share, inherit, and the change-log staging are serial.
- **Stylo's storage cut without Stylo's `Arc`s.** One pointer per style family; initial or parent until a declaration lands, then the node's own slot. Inherit is a parent hop, and an inherited change on a restyled node re-inherits its clean descendants.
- **Mutation writes.** Redis-shaped `CCExclHold` tickets (`domain.hold*` + `@destroy`).

## Gate and receipts

Same print format on both sides: `index tag id=… name=value ×189` in Stylo's order and serialization. `cmp` after stripping `#` lines is the gate; timings come after receipts match. See README “Fairness” for what was corrected to get here.

- Tiny suites and hand fixtures: `make compare` (81 / 81 after mutations, plus `fixtures/local/`).
- 20 k races: `make bench-style`, `make bench-{sibling,structural,nth,ba,media}`. Full dumps (70–200 MB) go to `receipts/full/` (gitignored); the committed receipt is the header plus a body digest.
- Local CSS (not StyleBench seeds): `fixtures/local/` — `make compare-local`. Fixture comments are `# ` only — a leading `#ident` is a CSS id selector.

## Status (2026-09-04)

All six suites `cmp`-clean at 20 k with the 189-column dump. CC is 2.7–7.6× faster on the first restyle and 1.5–30× on the mutation rounds (README “In short”). Length `calc()` (`px` / `em` / `rem` / `%` / numbers) is gated by `fixtures/local/calc.stylebench`. `:is()` / `:not()` / `:where()` by `fixtures/local/is-not.stylebench`. `var()` / `--*` by `fixtures/local/var.stylebench`. Box shorthands (1–4 sides, overflow / gap, `border`) by `fixtures/local/box.stylebench`. Not done: the rest of CSS (value lists, user-action, layout-time semantics) — none of it is exercised by StyleBench, so the output is identical there and would not be on arbitrary CSS.

## Ladybird (reference, not a gate)

The `ladybird/` submodule is a Distribution build of [LadybirdBrowser/ladybird](https://github.com/LadybirdBrowser/ladybird) run against the same vendored StyleBench (`scripts/browser-bench.sh ladybird`). It is not a third oracle — we do not `cmp` against Ladybird — but it is the closest in-browser analogue to what we built: a Rust `StyleEngine` (matching, invalidation routing, cascade identities) behind a C++ bridge that materializes `ComputedValues` for layout.

**Style-only clock.** Ladybird exposes `internals.getStyleInvalidationCounters().styleUpdateMicroseconds`, which brackets `Document::update_style` (style only, no layout). `scripts/browser-bench.sh ladybird 5 --internals` samples that around every StyleBench step and around the initial resolution (flush forced right after `createBenchmark()`). Receipt: `receipts/browser-ladybird-internals.txt`. On the edit rounds Ladybird's style pass is 1.0–4.4× Stylo and 2.5–8× slower than CC; on media it matches CC's scoped invalidation (55 ms vs Stylo's 425 ms). CC is 5.5–100× faster on the first restyle.

**Conservative runner.** Stock StyleBench's `getBoundingClientRect()` flush lets Ladybird skip geometry-neutral style work (~4 ms class/attr steps). `--conservative` injects Ladybird's own StyleBenchConservative `_runTest` (`browser-bench/conservative.mjs`); Chrome and WebKit are unchanged within noise.

**Engine replay (upstream, not working here yet).** Ladybird records StyleEngine boundary events during a StyleBench run (`LIBWEB_STYLE_RECORD`, `Meta/record-style-bench.py`) and replays them engine-only (`Build/distribution/bin/style-replay`). At submodule `a1db2e3a` replay of a full StyleBench trace fails: seven deferred-geometry event kinds are recorded but have no replay arms, then `publish_computed_groups` segfaults. Wrappers: `scripts/ladybird-style-record.sh`, `scripts/ladybird-style-replay.sh`. When upstream fixes replay, that is the closest apples-to-apples engine clock on their side.

**Plug-in seam (if pursued).** Replace the Rust `StyleEngine` behind `Libraries/LibWeb/Rust/StyleEngineBoundary.json` — feature journal in, ordered matches + invalidation reactions + StyleRecord/group payloads out. That is a whole-engine contract, not a drop-in matcher.
