# stylo-cc

A Concurrent-C styling engine raced against [Stylo](https://github.com/servo/stylo) on a frozen [StyleBench](https://perftest.netlify.app/stylebench/) workload.

```
fixtures/           generated StyleBench races (tiny / default / sibling / structural / nth / ba / media; gitignored)
fixtures/local/     hand CSS we own — still cmp vs Stylo
stylo-runner/       real Stylo (`style` crate) on a TElement host
engine/             idiomatic Concurrent-C engine; property layer generated at compile time (@comptime)
stylo/              git submodule — servo/stylo 0.20.0 (oracle crate + unit tests)
harness/            stylebench-gen — frozen WebKit StyleBench LCG / seeds
scripts/            bench-suite.sh — the 20 k sibling / structural / nth / ba races, the 5 k media race
                    gen-longhands.shcc — CC script: Stylo's longhand list → engine/longhands.cch
browser-bench/      StyleBench itself in Chrome / WebKit via Playwright, and in Ladybird (vendored copy gitignored)
ladybird/           git submodule — LadybirdBrowser/ladybird, built to Build/distribution by `scripts/browser-bench.sh ladybird-build`
receipts/           last cmp-clean receipts, both runners: full dumps for tiny / local, header + body digest for 20 k (full/ is gitignored)
```

Style only. No layout. No paint. No browser. Stylo submodule tracks `origin/main` (`b3e6425`).

## In short

**The test.** WebKit's StyleBench, frozen: a 5 000-rule stylesheet, a 20 000-element tree, and 25 rounds of DOM edits (add / remove classes, change attributes, add / remove leaves), each followed by a restyle. Six variants cover what real sheets do — plain descendant / child selectors, sibling combinators, `:first-child`-style structural selectors, `:nth-*` selectors, `::before` / `::after` pseudo-elements, and `@media` queries driven by 55 viewport resizes. Same seeds, same tree, same edits on both sides.

**The two engines.** Stylo — Firefox's and Servo's real style crate, release build, its thread pool and style sharing on — and this engine, written in Concurrent-C. Each takes the fixture text, styles the tree, runs the edits, and prints the computed style of every live element.

**The gate.** The two printouts must be byte-identical before any time is recorded. A row is one element (or `::before` / `::after` box) and all 189 CSS longhands Stylo exposes to `getComputedStyle`, in Stylo's order and Stylo's serialization — not just the properties the benchmark sheet happens to set. Every suite, every size, plus a set of hand-written fixtures (cascade order, `!important`, `em` / `rem` / `%`, `currentcolor`, `style=` attributes, inherited changes reaching untouched descendants) passes `cmp` clean.

**Completeness.** The property list is Stylo's own (`stylo-runner --longhands`), turned into code at compile time by a `@comptime` generator, so every element carries the full computed-style width: each of Stylo's 20 style structs is inherited or reset, every length resolved, every value printed. What the engine does not do is the rest of CSS — `calc()`, `var()`, value lists, `:not` / `:is`, user-action pseudo-classes, layout-time semantics. On these fixtures none of that is exercised, so the output is the same; on arbitrary CSS it would not be.

**The results** (Apple M5, wall clock, ms; lower is better):

| suite | Stylo first restyle | CC first restyle | Stylo 25 edits (55 resizes) | CC 25 edits (55 resizes) |
|---|---|---|---|---|
| default | 26.7 | **9.8** | 23.4 | **15.6** |
| sibling `+` `~` | 57.1 | **18.5** | 144.6 | **72.9** |
| structural `:first-child` … | 36.7 | **8.5** | 135.4 | **16.5** |
| nth `:nth-child(2n+1)` … | 39.9 | **10.1** | 166.1 | **38.1** |
| before / after | 36.3 | **11.0** | 57.5 | **18.7** |
| media (5 k elements) | 11.4 | **1.5** | 425.3 | **14.3** |

CC is 2.7–7.6× faster on the first restyle and 1.5–30× faster on the edit rounds, with identical output; the smallest margin is the default suite's edit rounds (1.5×), the largest is media resizes (30×), where Stylo restyles every element on each resize and CC restyles only those a flipped rule touches. Widening the dump from 11 properties to all 189 cost CC about half a millisecond per column; Stylo already computed all of them.

**The browsers, for scale.** The same StyleBench, unmodified, run in Chrome, in WebKit and in Ladybird on this machine (`scripts/browser-bench.sh`, receipts in `receipts/browser-*.txt`; WebKit is Playwright's build, not Safari's shipped one; Ladybird is a Distribution build of the `ladybird/` submodule). StyleBench times each edit step's JS + style recalc + **layout** in an 800×600 iframe and never times the initial resolution, so the only comparable column is the edit rounds, and the browser rows carry layout, UA-sheet matching, box construction and full-grammar CSS that ours do not. Order-of-magnitude only:

| suite | Chrome edits | WebKit edits | Ladybird edits | Stylo edits | CC edits |
|---|---|---|---|---|---|
| default | 118 | 99 | 211 *(118)* | 23.4 | 15.6 |
| sibling | 693 | 421 | 298 *(187)* | 144.6 | 72.9 |
| structural | 282 | 411 | 246 *(152)* | 135.4 | 16.5 |
| nth | 351 | 460 | 408 *(294)* | 166.1 | 38.1 |
| before / after | 200 | 163 | 339 *(350)* | 57.5 | 18.7 |
| media (55 resizes) | 446 | 270 | 207 *(202)* | 425.3 | 14.3 |

Ladybird's column is the **Conservative** runner; stock StyleBench is in parentheses. Stock StyleBench flushes each step with `getBoundingClientRect()` alone, and Ladybird's engine skips the style flush when it can prove the pending edits cannot move geometry — on stock, its class and attribute steps cost ~4 ms in every suite except before / after (where a class flip changes `content`), which is the restyle not being done, not being done fast. Ladybird's own perf harness closes that hole with a `_runTest` that also reads `getComputedStyle(body).backgroundColor` before and after each step; `--conservative` on either driver injects that exact override (`browser-bench/conservative.mjs`). Under it Chrome and WebKit are unchanged within noise (`receipts/browser-{chrome,webkit}-conservative.txt`), so their stock numbers stand. Stylo and CC have no lazy path: every StyleBench step runs the full restyle on the clock, so they are already at Conservative strictness or stricter.

A style-only probe on the same page (`--split`: force a full style flush, then `getBoundingClientRect` for layout) puts Chrome's default edit rounds at ~88 ms style / ~15 ms layout and the initial resolution StyleBench leaves untimed at ~37 ms, against Stylo's 26.7 and our 9.8 on the same tree. The probe is stable on the default suite and noisy elsewhere (WebKit's clock is 1 ms-granular), so only that row is quoted; full output is in `receipts/browser-chrome.txt` / `receipts/browser-webkit.txt`.

**Ladybird's own style clock.** Ladybird is the one browser here that times its style pass itself: with `--expose-internals-object`, `internals.getStyleInvalidationCounters().styleUpdateMicroseconds` brackets `Document::update_style` (style only, never layout), and sub-counters split it into the Rust `StyleEngine` (matching, invalidation, cascade identities) and the C++ recompute that materializes `ComputedValues`. `scripts/browser-bench.sh ladybird 5 --internals` samples those counters around every step and around a flush forced right after `createBenchmark()`, which is the initial resolution. That gives the first like-for-like column against Stylo and CC — same tree, same sheet, same edits, style only — with the caveat that Ladybird's pass still carries the UA sheet, full-grammar CSS and the layout-facing `ComputedValues` groups (min over 5 iterations, ms; `receipts/browser-ladybird-internals.txt`):

| suite | Ladybird first restyle | Stylo | CC | Ladybird 25 edits (55 resizes) | *of which Rust engine* | Stylo | CC |
|---|---|---|---|---|---|---|---|
| default | 92 | 26.7 | **9.8** | 103 | *88* | 23.4 | **15.6** |
| sibling | 102 | 57.1 | **18.5** | 182 | *165* | 144.6 | **72.9** |
| structural | 97 | 36.7 | **8.5** | 137 | *125* | 135.4 | **16.5** |
| nth | 105 | 39.9 | **10.1** | 292 | *257* | 166.1 | **38.1** |
| before / after | 129 | 36.3 | **11.0** | 132 | *100* | 57.5 | **18.7** |
| media (5 k elements) | 148 | 11.4 | **1.5** | 55 | *16* | 425.3 | **14.3** |

On the edit rounds Ladybird's style pass lands between 1.0× (structural) and 4.4× (default) of Stylo's time, and 8× faster than Stylo on media, where like CC it only restyles what a flipped rule touches. CC is 2.5–8× faster than Ladybird's style pass on the edits and 5.5–100× on the first restyle. The rest of Ladybird's StyleBench step — the gap between the `style` column and the sync column in the receipt — is JS and layout.

**What this is not.** A Firefox speedup. Stylo here runs on a small host (`stylo-runner/`) rather than in a browser, and our engine implements StyleBench's CSS, not CSS. The claim is narrower: on this workload, doing the same job to the same output, this shape is faster. The rest of this file is the detail — what each suite exercises, how each side matches and invalidates, what was corrected to make the race fair, and how to run it.

The gate is `cmp` of the style dump for every live element. Timings are wall clock after receipts match.

This is a StyleBench race against Stylo the crate, not a Firefox hook. A win here is not “faster Firefox.”

## What is being tested

Both runners eat the same text fixture and must print the same style dump.

**Sheet.** StyleBench `defaultConfiguration`: 5 000 rules. Combinators are descendant (` `) and child (`>`) only. Each generated rule sets `background-color: rgb(...)`. The base sheet sets `#testroot` font-size / line-height and `#testroot *` display / height / min-width. No `:nth-*`, no `::before` / `::after`, no media queries.

**Sibling suite.** Same generator with WebKit’s sibling knobs (`--sibling`): the rules also draw next-sibling `+` and subsequent-sibling `~` combinators, same tree LCG. Tiny sibling is on `make compare`; the 20 k sibling race is `make bench-sibling` (not on `make bench-style`, so default stays the headline).

**Structural suite.** WebKit’s “Structural pseudo classes” knobs (`--structural`): each compound draws one of `:first-child` / `:last-child` / `:first-of-type` / `:last-of-type` / `:only-of-type` / `:empty` with chance 0.1 (an element type is forced on that compound, as WebKit does). Leaf add / remove is what moves these: the new or gone leaf shifts the edge / only-of-type facts of its siblings and the `:empty` fact of its parent. Tiny structural is on `make compare`; the 20 k race is `make bench-structural`.

**Nth suite.** WebKit’s “Nth pseudo classes” knobs (`--nth`): each compound draws one of `:nth-child(2n+1)` / `:nth-last-child(3n)` / `:nth-of-type(3n)` / `:nth-last-of-type(4n)` with chance 0.1, type forced as above. A leaf add / remove now moves every following sibling’s parity and every same-type sibling’s count, so far more nodes change per mutation than in the structural suite. Tiny nth is on `make compare`; the 20 k race is `make bench-nth`.

**Before / after suite.** WebKit’s “Before and after pseudo elements” knobs (`--before-after`): the subject compound ends in `::before` or `::after` with chance 0.1, and that rule’s declaration also sets `content: ''; min-width: 5px; display: inline-block`. An element with a matching rule grows a pseudo box with its own cascade, inheriting from the element; Stylo drops a `::before` whose `content` is `none` / `normal`, so every kept box has effective content. Both dumps print the pseudo boxes after their element (`::before` then `::after`, plus `content=`); the 20 k race has 34 146 of them. Tiny is `make compare`; the 20 k race is `make bench-ba`.

**Media suite.** WebKit’s “Dynamic media queries” knobs (`--media`): 5 000 elements, no `*`, about 1 % of rules open an `@media (min-width | max-width: 300–700px)` block that closes with chance 0.3 per rule (the last block may run to EOF, as in WebKit). The steps are not DOM edits: each of the 5 steps resizes the viewport 300, 350, … 800 px — 55 restyles. Tiny adds one trailing resize to 450 so the final dump sees a toggled state (every WebKit step ends back at 800). Tiny is `make compare`; the race is `make bench-media`.

**Tree.** Same LCG as WebKit (`styleSeed=1`, `domSeed=2`). Default is 20 000 generated elements plus `#testroot` (20 001 nodes). Tags, ids, classes, and attributes are the StyleBench random draw (class pool 200, id chance 0.05, …).

**First restyle (`TIME_MS`).** Match live nodes and cascade every content longhand (189 — the width Stylo exposes to `getComputedStyle`). Stylo: `recalc_style_at` via `traverse_dom` on Servo’s `STYLE_THREAD_POOL` (cap 6), 32-entry sharing LRU on. CC: subject / pair inverted maps; 256-bit blooms for self, ancestors, and preceding siblings so `match_from` short-circuits ` ` / `>` / `~` walks (each rule also carries the union need of its ancestor compounds, one test against the subject’s ancestor bloom before any walk); position bits per node — one per distinct position predicate the sheet uses (`:first-child` is the row `nth-child(0n+1)`; `:nth-child(2n+1)` is another row; at most 32), computed from the node’s four sibling counts, matched like any other compound fact and part of the share key; a `::before` / `::after` rule is indexed in the same subject buckets and cascades into the element’s pseudo box (a second node record per side, same cascade code, inherits from the element) instead of the element; exact sibling share (same parent + interned tag / id / classes / attrs + sibling-rule signature) then `@parallel for` over canons in worker-sized arms — each canon collects its matched rules, sorts them once by (origin, specificity, order) and applies the declarations, normal then `!important`; inherited families hop the parent in one serial depth walk, and a node whose inherited struct came out different sends its clean children through that hop again (an ancestor's `color` or `font-size` flip reaches descendants that matched no new rule, and their `em`s re-resolve). After that, a dirty restyle re-resolves share from the sibling list and skips `paint_levels` unless a leaf was added.

**Sibling-rule signature.** Rules whose last combinator is `+` / `~` are indexed apart. Before share, each node records which of those rules match it (up to 8; overflow opts the node out of share). That list is part of the share key, so two twins share only if their sibling context agrees — the same hole Stylo’s revalidation selectors close. No property is special-cased.

**Mutations (`TIME_MUT_MS`).** The official StyleBench step loop, frozen in the fixture (same DOM LCG, same skip rules):

```
for step in 0..5:          # tiny: 1 step, 8 ops each
    addClasses(100)
    restyle
    removeClasses(100)
    restyle
    mutateAttributes(100)
    restyle
    addLeafElements(100)
    restyle
    removeLeafElements(100)
    restyle
```

That is 25 restyles on default, sibling, structural, nth, and before / after (5 on tiny); the media suite is 55 resizes instead. After each batch Stylo snapshots the old class / attr set, walks dirty bits up to `#testroot`, and lets `invalidate_style_if_needed` choose `RESTYLE_SELF` vs descendants; on leaf add / remove the host applies Gecko’s `ElementSelectorFlags` rules (`HAS_SLOW_SELECTOR`, `HAS_SLOW_SELECTOR_LATER_SIBLINGS`, `HAS_EDGE_CHILD_SELECTOR`, `HAS_EMPTY_SELECTOR`) to pick which siblings and parent to restyle. CC records each change at `apply_mut` (atom, slot, old / new value; a position-bit flip on a sibling is a change too), marks the mutated node dirty, groups the log by node, then per changed node with a live child (or with `+` / `~` in the sheet — otherwise nothing below it can flip) collects the rules whose left compound holds a changed atom and matched that node — in the one state where the node has the atom — and its candidates (descendants, following siblings, both, by the rule’s reach). Every candidate gets a signature of the rules it matches with the whole batch applied, then again with the whole batch undone; a flip dirties it. The signature passes run `@parallel for` over all candidates of all changed nodes (they only read the tree); staging is serial. Undo / redo is positional — `removeClass` takes `classes[0]` and fixtures carry duplicate attributes, so a set-only inverse would drift. Blooms repaint only what moved: changed nodes (self as a superset of both states, so the undone pass may prune with it), their subtrees, their sibling lists. The batch form matters: two attrs on nested ancestors in one restyle can flip a `[attr] type` join that no single change flips (`fixtures/local/attr-desc-batch.stylebench`). Sibling share copies specified values into the node’s own slots so inherit cannot write through a clean canon. `TIME_MUT_MS` is the sum of those restyles **including invalidation** (change-log flip, bloom repaint) — only applying the DOM edits is outside the clock. On a resize, Stylo swaps the `Device`, rebuilds the origins whose media-affected rules moved (`Stylist::set_device`, `flush`), and restyles from the root, as Servo does; CC’s fact is per rule — its media row flipped — and the nodes that fact touches are exactly the ones matching that rule, so the flipped rules become one subject-keyed table and every live node takes one signature pass over it in parallel (rules forced on for the pass), dirty on any hit. Earlier receipts ran CC invalidation off the clock; those numbers are not comparable.

**Not measured.** Layout, paint. Resize is measured only for what it does to style (media queries), not layout. CC invalidation is not Stylo’s snapshot + invalidation map: a change log and a batch `match_from` flip. Under `+` / `~`, adding or removing a leaf dirties its following siblings (their sibling signature may change); the dirty set is then rematched, not diffed.

**Correctness.** After the last mutation restyle, both runners print

```
index<TAB>tag<TAB>id=…<TAB>align-items=normal<TAB>alignment-baseline=baseline<TAB>…<TAB>z-index=auto
```

one `name=value` per content longhand (189 columns, Stylo's id order, Stylo's serialization: `rgb(r, g, b)`, `20px`, `"quoted"` content) for live elements and their kept `::before` / `::after` boxes (stable fixture ids; removed leaves omitted). Stylo's side is `computed_or_resolved_value` per longhand; `stylo-runner --longhands` prints the list the CC side is generated from. `make compare` / `make bench-style` strip `#` lines and `cmp` the dumps.

## Local CSS

Hand fixtures under `fixtures/local/` (not gitignored). Same `---base---` / `---css---` / `---tree---` / `---mut---` text. `make compare-local` loops them. Do not change tiny/default seeds — that is the race.

Fixture comments are `# ` only — a leading `#ident` is a CSS id selector. Tree rows need trailing tabs for empty class/attr cells.

| file | what it gates |
|---|---|
| `cascade.stylebench` | `2em`, `#rgb` / `#rrggbb`, `!important`, sibling share + em |
| `computed.stylebench` | `font-size` `%`, `currentColor` on bg, `color: currentColor` as inherit |
| `attr.stylebench` | `style=` vs author / author important / style important; `2rem` vs nested `2em` |
| `shorthand.stylebench` | `background` → `background-color` (omitted color → transparent), same-rule order, `style=background:…` |
| `sibling.stylebench` | next-sibling `+`, subsequent `~`, mixed `>` + `+`, class / leaf muts |
| `attr-desc.stylebench` | `[attr] type` descendant after an ancestor attr mut |
| `attr-desc-batch.stylebench` | two ancestor attrs in one restyle — the join flips only as a batch |
| `inherit-prop.stylebench` | inherited `color` / `font-size` flip on an ancestor (class, then `style=`) reaches clean descendants; `1em` two levels down re-resolves |

## Properties

All 189 longhands Stylo exposes to content are cascaded, inherited, resolved and dumped. The list is Stylo's (`stylo-runner --longhands`: `LonghandId` minus internal / disabled), not ours. No runtime table walks it:

- `scripts/gen-longhands.shcc` (Concurrent-C in script mode; SERDES `@grammar` for the TSV and for `longhands.toml`) writes `engine/longhands.cch`: one row per longhand — name, Stylo style struct, inherited flag, value kind, initial value text.
- `engine/sty_emit.cch` is one `@comptime` function. At compile time it reads that table and emits the property layer as C: a `Sty_<family>` struct per Stylo style struct (20: `box`, `font`, `inherited_text`, `background`, `position`, …), `INIT` literals, `P_*` / `A_*` enums, `prop_by_name`, and per family the inherit / compare / resolve / own-on-write / apply functions; `Sty_dump` is 189 straight-line `css_out_*` calls in Stylo's order. The engine includes it with one line.
- Adding a longhand is one row in the table (re-run the script when Stylo's list moves); adding a value kind is one cell type in `engine/cssval.cch`.

Value cells (`engine/cssval.cch`, plain C so the `@comptime` emitter and the runtime share them): an **atom** (interned keyword, or the text kept verbatim), a **length** (number + `px` / `em` / `rem` / `%`, or `auto` / `normal` / `none` / …; the specified number is kept so a re-inherit re-resolves exactly), a **color** (`rgb()` / `rgba()` / `#rgb` … `#rrggbbaa` / `transparent` / `currentcolor`). `font-size` `%` and `em` resolve against the parent, `rem` against the root; `currentcolor` on `color` is the inherited color.

Storage is Stylo's cut without Stylo's `Arc`s: a node holds one pointer per family. An unspecified family points at the initial struct (reset) or the parent's (inherited); the first declaration that lands in a family copies what the node was viewing into the node's own slot and writes there. A share twin clones the canon's owned structs into its own slots — not aliased, because incremental inherit is dirty-only and a dirty twin must not write through a clean canon. Pools exist only for families the sheet, `style=` attributes, or mutation values declare into.

Cascade: UA / author / `style=` origin, `!important` (author important < style important < UA important), specificity, source order; `background` shorthand → its longhands; CSS-wide `initial`.

Selectors: type, `#id`, `.class`, `[attr]` / `[attr=val]`, `*`, descendant, child, next-sibling (`+`), subsequent-sibling (`~`), `:first-child` / `:last-child` / `:first-of-type` / `:last-of-type` / `:only-of-type` / `:empty`, `:nth-child(an+b)` / `:nth-last-child(an+b)` / `:nth-of-type(an+b)` / `:nth-last-of-type(an+b)` (`odd` / `even` too; no whitespace inside the parens, no `of S`), `::before` / `::after` on the subject. At-rules: `@media (min-width | max-width: Npx)` blocks, evaluated against the viewport width (800 px until a `resize` step).

**Not in this engine.** The rest of CSS. In particular:

- **Selectors:** `:only-child`, `:nth-*(… of S)`, `:not` / `:is` / `:where`, `:hover` and other user-action, `::first-line` / `::first-letter` and other pseudo-elements, media features beyond width, `@supports`, shadow, namespaces.
- **Values:** `calc()`, `var()`, hsl, named colors beyond `black` / `white` / `red` / `transparent` / `currentColor`, multi-value lists (`font-family` stacks, `transform` functions), shorthands other than `background`. A declaration the cell parser rejects is dropped, as an invalid declaration would be — so the width is Stylo's, the value grammar is StyleBench's.
- **Semantics** past the cascade: no `display` blockification, no `writing-mode` logical mapping, no `text-decoration` propagation, no `font-size` keyword table.

## Recorded times

Release, wall clock, Apple M5, 2026-09-04, 189-longhand dump on both sides. Live after mutations: 20 002 (media: 5 001). Sharing on both sides. Inherit on CC is serial. Stylo: snapshots + invalidation map (mutation clock includes it). CC: change log → staged batch `match_from` flip, signature passes parallel, invalidation on the clock; dirty restyle re-resolves share from the sibling list. `ccc` 0.3.4-319 (`-O`, 10 workers). Stylo `0.20.0` (`b3e6425`). `cmp` clean on every row.

**Default** (` ` / `>` only, `make bench-style`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 26.7 ms | 23.4 ms |
| Stylo (6 threads, sharing off) | 45.3 ms | 1099 ms |
| CC (`ccc -O`) | 9.8 ms | 15.6 ms |

**Sibling** (` ` / `>` / `+` / `~`, `make bench-sibling`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 57.1 ms | 144.6 ms |
| CC (`ccc -O`) | 18.5 ms | 72.9 ms |

**Structural** (` ` / `>` + `:first-child` … `:empty`, `make bench-structural`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 36.7 ms | 135.4 ms |
| CC (`ccc -O`) | 8.5 ms | 16.5 ms |

**Nth** (` ` / `>` + `:nth-child(2n+1)` … `:nth-last-of-type(4n)`, `make bench-nth`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 39.9 ms | 166.1 ms |
| CC (`ccc -O`) | 10.1 ms | 38.1 ms |

**Before / after** (` ` / `>` + `::before` / `::after` subjects, `make bench-ba`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 36.3 ms | 57.5 ms |
| CC (`ccc -O`) | 11.0 ms | 18.7 ms |

**Media** (5 000 elements, `@media` blocks, 55 viewport resizes, `make bench-media`):

| | first restyle | 55 resize restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 11.4 ms | 425.3 ms |
| CC (`ccc -O`) | 1.5 ms | 14.3 ms |

Sharing-off is from `receipts/default_2026_08_29.txt` (full rematch, not re-run). Everything else is this rebuild (`receipts/default.*.txt`, `receipts/sibling20k.*.txt`, `receipts/structural20k.*.txt`, `receipts/nth20k.*.txt`, `receipts/ba20k.*.txt`, `receipts/media5k.*.txt`). The earlier CC default mutation number (11.7 ms) ran invalidation off the clock. Widening the dump from 11 properties to all 189 longhands cost CC about half a millisecond per column on the same machine (default 9.3 → 9.8 first restyle, 15.2 → 15.6 mutations; the 11-property engine rebuilt and run side by side): the extra width is a pointer per family plus one struct clone per share twin, not per-property work. Stylo's numbers did not move — it computed all 189 already; only the dump got wider. Tiny (81/81) is `make compare` (also runs `fixtures/local/`); that target builds Stylo debug, so its `TIME_*` lines are not the recorded numbers. Warm `-O` first-restyle `TIME_MS` is noisy if the binary is cold — `bench-sibling` / `bench-structural` / `bench-nth` / `bench-ba` / `bench-media` do a warm run first; for `bench-style` record the second run. Run-to-run spread on CC is about ±1 ms default / structural / nth, ±3 ms sibling; a busy machine can double a Stylo row, so re-run any row that looks off.

CC split (warm `-O`; the `TIME_*` / `MUT_*` fields in the receipt). Default: first match 7.2 (collect hits, sort, apply 189 wide), share 1.3, inherit 0.8, **12 302 canons**; the 25 mutation restyles dirty 7 311 nodes in total — invalidation 4.5 (`MUT_FANOUT`: the 480 changed nodes with children are staged, 54 348 candidates; the two signature passes are ~1 of it, staging the rest), bloom repaint 1.2, match 4.0, canon 1.8, twin copy 1.2, inherit 0.5, sig 0.6. Structural: first match 5.6, **13 890 canons** (position bits split some twins); mutations dirty 7 975 nodes — invalidation 4.8 (497 staged), match 4.4. Nth: first match 6.4, **15 543 canons**; mutations dirty 34 654 nodes (a leaf op flips the parity bit of about half its following siblings, and a flipped node is dirty) — invalidation 10.4 (1 219 staged; flipped leaves have nothing below them), match 13.3, canon 5.7. Before / after: first match 7.8, **12 302 canons** (pseudo boxes ride the element’s share); mutations dirty 11 582 nodes — invalidation 4.8, match 5.6, canon 2.2. Media: first match 1.0 on 5 001 nodes, **2 957 canons**; the 55 resizes toggle 1 130 rules in total and dirty 22 405 nodes — invalidation 3.2 (the one signature pass over all live nodes per resize), match 4.2, canon 3.1, inherit 0.7 (the inherited-change snapshot is the family pointers plus the values of families the node owns, not every inherited struct by value); Stylo restyles all 5 000 elements 55 times. Sibling: first match 7.2, share 10.0, **14 447 canons** (fewer twins agree once sibling context is in the key); mutations dirty 44 526 nodes — sig 24.3, invalidation 16.9 (every changed node is staged: `+` / `~` reach across), match 15.4, canon 7.2. The signature pass is the cost of `+` / `~` here: it runs on dirty nodes only, but leaf add / remove dirties every following sibling. Dirty restyle does not qsort the slab or rebuild levels unless a leaf was added.

Parallel arms are runs of nodes, not single nodes. `ccc` gates spawn on measured leaf cost (~8 µs); a one-node arm sits on that line and the verdict flips per run, which showed up as ±10 ms on the sibling suite. Arm size is ≥ 32 nodes, else range ÷ (4 × workers).

## Fairness

Corrected relative to the first receipts (RGB-only dump, Stylo sharing off, CC dropping the base sheet, fake-parallel inherit, an 11-property dump, inherited changes that stopped at the dirty node):

- **Same dump, full width.** Every content longhand Stylo has (189), in Stylo's order and serialization, for elements and kept pseudo boxes. The per-element burden is the real one — every family inherited or reset, every length resolved, every value printed — not the eleven properties StyleBench happens to touch. Base sheet is cascaded on both sides. `cmp` is the gate.
- **Sharing on.** Stylo’s 32-entry LRU (sibling + cousin after revalidation). CC exact same-parent identity plus the sibling-rule signature (12 302 canons of 20 002 on default, 13 890 on structural, 15 543 on nth, 12 302 on before / after, 14 447 on sibling, 2 957 of 5 001 on media). Share is all-or-nothing on both sides; no property is rematched per node.
- **Inherit is a parent hop, and it propagates.** Specified match stays `@parallel for` over canons. Inherited families copy in one serial depth walk — not one `@parallel for` per level. A restyled node whose inherited struct came out different marks its clean children, which re-inherit (and their children, while the change carries). Before this, an ancestor's `color` flip never reached a descendant that matched no new rule; Stylo always did this. `fixtures/local/inherit-prop.stylebench` gates it — and exposed that the Stylo host ignored a `style=` mutation, since the style attribute is not a selector input; `host.rs` now posts `RESTYLE_STYLE_ATTRIBUTE` there, as Gecko does.
- **Same mutation script, same clock.** Stylo takes element snapshots and runs the crate invalidator inside `TIME_MUT_MS`. CC logs the changes at mutate and, inside the same clock, rematches a descendant or following sibling only if its rule signature flips, batch applied vs batch undone, over rules keyed by the changed atoms. Undo / redo is positional so the applied state is exactly what Stylo’s host sees (first duplicate attribute replaced, `classes[0]` removed).
- **Competitor is Stylo the crate**, not the `TElement` host (`host.rs` is glue, like the CC fixture load).

Still not the same program:

- Stylo’s proto is `Arc` style structs (atomic refcount) plus a rule tree. Ours is a pointer per family to initial / parent / this node’s slot, plus a per-canon sorted hit list. Same 20-struct inherit-vs-reset cut, same COW-on-first-write; we do not pay atomics or a rule-tree walk.
- Stylo parses and computes the full value grammar (`calc()`, lists, every keyword). We parse what StyleBench and the local fixtures write — a value we cannot parse is dropped as invalid. On these fixtures that is the same computed style; on arbitrary CSS it would not be.
- Stylo's property layer is generated by Mako at build time from `longhands.toml`; ours by a `@comptime` function from the same list. Neither side hand-writes 189 properties.
- When an inherited value changes, Stylo recascades the children (the full cascade against the new parent); we re-inherit them (a pointer hop per family, re-resolve of owned lengths). Same computed style, less work per node — a shape difference, not a burden dropped.
- Sharing algorithms differ (LRU + revalidation selectors vs exact sibling key + sibling-rule signature).
- Stylo walks in tree order; we match canons then hop inherit.
- `TIME_*` is `traverse_dom` / `recalc_style_at` vs our restyle. Building the slab / parsing the fixture is outside the clock.

A win on this receipt is not “a faster Stylo.”

## Setup

Rust stable and the `stylo/` submodule. The CC runner needs [`ccc`](https://github.com/sreekotay/concurrent-c).

```bash
source "$HOME/.cargo/env"
git submodule update --init --depth 1
```

## Run

```bash
make fixture          # tiny suite (compile / cmp loop)
make compare          # tiny + tiny-sibling/structural/nth/ba/media + fixtures/local: both runners, cmp styles
make compare-local    # only fixtures/local (no StyleBench regenerate)
make compare-sibling  # generated tiny sibling combinators vs Stylo
make compare-structural # generated tiny structural pseudo-classes vs Stylo
make compare-nth      # generated tiny :nth-* pseudo-classes vs Stylo
make compare-ba       # generated tiny ::before / ::after vs Stylo
make compare-media    # generated tiny @media + resize steps vs Stylo
make bench-style      # default 20k/5k + mutations, release Stylo + CC -O, cmp then times
make bench-sibling    # sibling 20k/5k, release Stylo + warm CC -O, cmp then times
make bench-structural # structural 20k/5k, same shape
make bench-nth        # nth 20k/5k, same shape
make bench-ba         # before/after 20k/5k, same shape
make bench-media      # media queries 5k elements, 55 resizes, same shape
make longhands        # regenerate engine/longhands.cch from Stylo (--longhands dump + longhands.toml)
scripts/browser-bench.sh all 5   # StyleBench in Chrome + Playwright WebKit (+ Safari if remote automation is on)
scripts/browser-bench.sh ladybird-build          # build the ladybird/ submodule (Distribution; 30+ min first time)
scripts/browser-bench.sh ladybird 5 --conservative   # StyleBench in Ladybird; drop --conservative for the stock runner
scripts/browser-bench.sh ladybird 5 --internals      # + Ladybird's own update_style clock per step and for the initial resolution
```

`CC_STYLE_WORKERS=n` caps the CC worker pool (default: all cores).

`make test` is **upstream Stylo crate tests** (`cargo test --workspace` in `stylo/`). We did not write those. The race fixtures are a frozen [WebKit StyleBench](https://perftest.netlify.app/stylebench/) port (`harness/` = `stylebench-gen`, same LCG / seeds). Stylo is the other runner, not the source of tiny/default.

Add more CSS locally by dropping a `.stylebench` file under `fixtures/local/` (same section markers; both runners already eat it). Do not change tiny/default seeds. `fixtures/*.stylebench` is gitignored; `fixtures/local/` is not.
