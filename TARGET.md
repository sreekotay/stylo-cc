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

## Gate and receipts

Same print format on both sides: `index tag id=… name=value ×189` in Stylo's order and serialization. `cmp` after stripping `#` lines is the gate; timings come after receipts match. See README “Fairness” for what was corrected to get here.

- Tiny suites and hand fixtures: `make compare` (81 / 81 after mutations, plus `fixtures/local/`).
- 20 k races: `make bench-style`, `make bench-{sibling,structural,nth,ba,media}`. Full dumps (70–200 MB) go to `receipts/full/` (gitignored); the committed receipt is the header plus a body digest.
- Local CSS (not StyleBench seeds): `fixtures/local/` — `make compare-local`. Fixture comments are `# ` only — a leading `#ident` is a CSS id selector.

## Status (2026-09-04)

All six suites `cmp`-clean at 20 k with the 189-column dump. CC is 2.7–7.6× faster on the first restyle and 1.5–30× on the mutation rounds (README “In short”). Not done: the rest of CSS (`calc()`, `var()`, value lists, `:not` / `:is`, layout-time semantics) — none of it is exercised by StyleBench, so the output is identical here and would not be on arbitrary CSS.
