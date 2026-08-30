# Target

Two standalone runners, one frozen [StyleBench](https://github.com/WebKit/WebKit/blob/main/PerformanceTests/StyleBench/resources/style-bench.js) workload.

```
StyleBench workload generator / fixtures
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

Stylo is the competitor: real matching, cascade, `RuleTree`, `recalc_style_at`, hosted on a slab tree that implements `TElement` / `TNode`.

The CC runner is a styling engine (tree, match, cascade, computed values, walk) written the [CC way](https://github.com/sreekotay/concurrent-c/blob/main/docs/the-cc-way.md). Sequential first; parallel once the relations are nameable. Mutation writes use Redis-shaped shard holds.

Same print format. `cmp` is the gate (dumped properties: display / position / width / height / min-width / font-size / line-height / font-weight / visibility / color / background-color). Timings come after receipts match. See README “Fairness.”

Tiny: 81/81 after mutations (`receipts/tiny_2026_08_29.txt`).
Default wall times: `receipts/default_2026_08_29.txt`.
Fixture comments are `# ` only — a leading `#ident` is a CSS id selector.
