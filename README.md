# stylo-cc

A Concurrent-C styling engine raced against [Stylo](https://github.com/servo/stylo) on a frozen [StyleBench](https://perftest.netlify.app/stylebench/) workload.

```
fixtures/           generated StyleBench races (tiny / default / sibling / structural; gitignored)
fixtures/local/     hand CSS we own — still cmp vs Stylo
stylo-runner/       real Stylo (`style` crate) on a TElement host
engine/             idiomatic Concurrent-C engine
stylo/              git submodule — servo/stylo 0.20.0 (oracle crate + unit tests)
harness/            stylebench-gen — frozen WebKit StyleBench LCG / seeds
scripts/            bench-suite.sh — the 20 k sibling / structural races
receipts/           last cmp-clean dumps + TIME lines, both runners
```

Style only. No layout. No paint. No browser. Stylo submodule tracks `origin/main` (`b3e6425`).

The gate is `cmp` of the style dump for every live element. Timings are wall clock after receipts match.

This is a StyleBench race against Stylo the crate, not a Firefox hook. A win here is not “faster Firefox.”

## What is being tested

Both runners eat the same text fixture and must print the same style dump.

**Sheet.** StyleBench `defaultConfiguration`: 5 000 rules. Combinators are descendant (` `) and child (`>`) only. Each generated rule sets `background-color: rgb(...)`. The base sheet sets `#testroot` font-size / line-height and `#testroot *` display / height / min-width. No `:nth-*`, no `::before` / `::after`, no media queries.

**Sibling suite.** Same generator with WebKit’s sibling knobs (`--sibling`): the rules also draw next-sibling `+` and subsequent-sibling `~` combinators, same tree LCG. Tiny sibling is on `make compare`; the 20 k sibling race is `make bench-sibling` (not on `make bench-style`, so default stays the headline).

**Structural suite.** WebKit’s “Structural pseudo classes” knobs (`--structural`): each compound draws one of `:first-child` / `:last-child` / `:first-of-type` / `:last-of-type` / `:only-of-type` / `:empty` with chance 0.1 (an element type is forced on that compound, as WebKit does). Leaf add / remove is what moves these: the new or gone leaf shifts the edge / only-of-type facts of its siblings and the `:empty` fact of its parent. Tiny structural is on `make compare`; the 20 k race is `make bench-structural`.

**Tree.** Same LCG as WebKit (`styleSeed=1`, `domSeed=2`). Default is 20 000 generated elements plus `#testroot` (20 001 nodes). Tags, ids, classes, and attributes are the StyleBench random draw (class pool 200, id chance 0.05, …).

**First restyle (`TIME_MS`).** Match live nodes and cascade the dumped properties. Stylo: `recalc_style_at` via `traverse_dom` on Servo’s `STYLE_THREAD_POOL` (cap 6), 32-entry sharing LRU on. CC: subject / pair inverted maps; 256-bit blooms for self, ancestors, and preceding siblings so `match_from` short-circuits ` ` / `>` / `~` walks (each rule also carries the union need of its ancestor compounds, one test against the subject’s ancestor bloom before any walk); structural bits per node (`first-child` … `empty`), masked to the pseudos the sheet uses, matched like any other compound fact and part of the share key; exact sibling share (same parent + interned tag / id / classes / attrs + sibling-rule signature) then `@parallel for` over canons in worker-sized arms; inherited properties hop the parent in one serial depth walk. After that, a dirty restyle re-resolves share from the sibling list and skips `paint_levels` unless a leaf was added.

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

That is 25 restyles on default, sibling, and structural (5 on tiny). After each batch Stylo snapshots the old class / attr set, walks dirty bits up to `#testroot`, and lets `invalidate_style_if_needed` choose `RESTYLE_SELF` vs descendants; on leaf add / remove the host applies Gecko’s `ElementSelectorFlags` rules (`HAS_SLOW_SELECTOR_LATER_SIBLINGS`, `HAS_EDGE_CHILD_SELECTOR`, `HAS_EMPTY_SELECTOR`) to pick which siblings and parent to restyle. CC records each change at `apply_mut` (atom, slot, old / new value; a position-bit flip on a sibling is a change too), marks the mutated node dirty, then per changed node collects the rules whose left compound holds a changed atom and matched that node — in the one state where the node has the atom — and its candidates (descendants, following siblings, both, by the rule’s reach). Every candidate gets a signature of the rules it matches with the whole batch applied, then again with the whole batch undone; a flip dirties it. The signature passes run `@parallel for` over all candidates of all changed nodes (they only read the tree); staging is serial. Undo / redo is positional — `removeClass` takes `classes[0]` and fixtures carry duplicate attributes, so a set-only inverse would drift. Blooms repaint only what moved: changed nodes (self as a superset of both states, so the undone pass may prune with it), their subtrees, their sibling lists. The batch form matters: two attrs on nested ancestors in one restyle can flip a `[attr] type` join that no single change flips (`fixtures/local/attr-desc-batch.stylebench`). Sibling share copies specified values into the node’s own slots so inherit cannot write through a clean canon. `TIME_MUT_MS` is the sum of those restyles **including invalidation** (change-log flip, bloom repaint) — only applying the DOM edits is outside the clock. Earlier receipts ran CC invalidation off the clock; those numbers are not comparable.

**Not measured.** Layout, paint, resize, nth suite, `::before` / `::after`. CC invalidation is not Stylo’s snapshot + invalidation map: a change log and a batch `match_from` flip. Under `+` / `~`, adding or removing a leaf dirties its following siblings (their sibling signature may change); the dirty set is then rematched, not diffed.

**Correctness.** After the last mutation restyle, both runners print

```
index<TAB>tag<TAB>id=…<TAB>disp=…<TAB>pos=…<TAB>w=…<TAB>h=…<TAB>minw=…<TAB>fs=…<TAB>lh=…<TAB>fw=…<TAB>vis=…<TAB>color=rgb(r, g, b)<TAB>bg=r,g,b,a
```

for live elements (stable fixture ids; removed leaves omitted). `make compare` / `make bench-style` strip `#` lines and `cmp` the dumps.

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

## Properties

**Cascaded and dumped** (StyleBench sheet + unexercised initials, so the dump is not only `background-color`):

| property | inherit? | notes |
|---|---|---|
| `display` | no | HTML `div` → `block`, else `inline`; base sheet `inline-block` |
| `position` | no | initial `static` |
| `width` | no | initial `auto` |
| `height` | no | initial `auto`; base sheet `10px` |
| `min-width` | no | initial `auto`; base sheet `10px` |
| `font-size` | yes | initial `16px`; `#testroot` `10px`; `px` / `em` / `rem` / `%` |
| `line-height` | yes | initial `normal`; `#testroot` `10px` |
| `font-weight` | yes | initial `400` |
| `visibility` | yes | initial `visible` |
| `color` | yes | initial `rgb(0, 0, 0)`; `rgb()` / `#rgb` / `#rrggbb` |
| `background-color` | no | generated sheet; dumped as `r,g,b,a` |

**Also computed** (local fixtures; Stylo already does these):

- Cascade: UA / author / style-attribute origin, `!important` (style important beats author important; UA important wins).
- `style=` is style origin (after share copy, serial apply).
- `background` shorthand → `background-color` (no color token → transparent). Other background longhands are not dumped.
- `currentColor` on `background-color` (resolved after inherit of `color`). On `color`, it inherits.
- Hex `#rgb` / `#rrggbb` and `transparent` on color / background-color.

Selectors: type, `#id`, `.class`, `[attr]` / `[attr=val]`, `*`, descendant, child, next-sibling (`+`), subsequent-sibling (`~`), `:first-child` / `:last-child` / `:first-of-type` / `:last-of-type` / `:only-of-type` / `:empty`.

**Not in this engine.** The rest of CSS. In particular:

- **Selectors:** `:nth-*`, `:only-child`, `:not` / `:is` / `:where`, `:hover` and other user-action, `::before` / `::after`, media / supports, shadow, namespaces.
- **Properties:** margin, padding, border, flex, grid, transform, animation, `font-family` / `font-style`, text-*, overflow, z-index, opacity, box-sizing, white-space, vertical-align, max-*, min-height, insets, float, …
- **Values:** `calc()`, `var()`, hsl, `%` on box lengths. Named colors beyond `transparent` / `currentColor`.

Stylo does not walk every longhand. Unused properties stay on a **proto** (parent `Arc` if inherited, initial `Arc` if reset) and are only COW’d when a declaration hits that struct. StyleBench dirties Box + Background on almost every node; Font is specified on `#testroot` and borrowed by kids. We use the same cut: `StyBg*` / `StyBox*` / `StyFont*` — initial or parent proto, unique slot on first specified write (no memcpy of the proto). The leftover tilt is Stylo’s `Arc` bag (and rule tree), not flex/grid we skipped.

## Recorded times

Release, wall clock, Apple M5, 2026-09-04. Live after mutations: 20 002. Sharing on both sides. Inherit on CC is serial. Stylo: snapshots + invalidation map (mutation clock includes it). CC: change log → staged batch `match_from` flip, signature passes parallel, invalidation on the clock; dirty restyle re-resolves share from the sibling list. `ccc` 0.3.4-319 (`-O`, 10 workers). Stylo `0.20.0` (`b3e6425`). `cmp` clean on every row.

**Default** (` ` / `>` only, `make bench-style`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 31.3 ms | 24.0 ms |
| Stylo (6 threads, sharing off) | 45.3 ms | 1099 ms |
| CC (`ccc -O`) | 5.9 ms | 18.0 ms |

**Sibling** (` ` / `>` / `+` / `~`, `make bench-sibling`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 56.7 ms | 140.9 ms |
| CC (`ccc -O`) | 15.2 ms | 67.0 ms |

**Structural** (` ` / `>` + `:first-child` … `:empty`, `make bench-structural`):

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 37.4 ms | 132.2 ms |
| CC (`ccc -O`) | 7.7 ms | 20.8 ms |

Sharing-off is from `receipts/default_2026_08_29.txt` (full rematch, not re-run). Everything else is this rebuild (`receipts/default.*.txt`, `receipts/sibling20k.*.txt`, `receipts/structural20k.*.txt`). The earlier CC default mutation number (11.7 ms) ran invalidation off the clock; 18.0 is the honest figure. Tiny (81/81) is `make compare` (also runs `fixtures/local/`); that target builds Stylo debug, so its `TIME_*` lines are not the recorded numbers. Warm `-O` first-restyle `TIME_MS` is noisy if the binary is cold — `bench-sibling` / `bench-structural` do a warm run first; for `bench-style` record the second run. Run-to-run spread on CC is about ±0.5 ms default / structural, ±2 ms sibling.

CC split (warm `-O`; the `TIME_*` / `MUT_*` fields in the receipt). Default: first match 4.2, share 1.3, inherit 0.12, **12 302 canons**; the 25 mutation restyles dirty 7 311 nodes in total — invalidation 9.1 (`MUT_FANOUT`: 1 486 changed nodes, 54 348 candidates, 99 347 rule tests staged; the two signature passes are ~3 of it, staging the rest), bloom repaint 1.1, match 2.9, canon 1.7, sig 0.8. Structural: first match 6.2, **13 890 canons** (position bits split some twins); mutations dirty 7 975 nodes — invalidation 9.3, match 4.1. Sibling: first match 5.3, share 10.4, **14 447 canons** (fewer twins agree once sibling context is in the key); mutations dirty 44 526 nodes — sig 24.1, invalidation 16.5, match 13.6, canon 7.1. The signature pass is the cost of `+` / `~` here: it runs on dirty nodes only, but leaf add / remove dirties every following sibling. Dirty restyle does not qsort the slab or rebuild levels unless a leaf was added.

Parallel arms are runs of nodes, not single nodes. `ccc` gates spawn on measured leaf cost (~8 µs); a one-node arm sits on that line and the verdict flips per run, which showed up as ±10 ms on the sibling suite. Arm size is ≥ 32 nodes, else range ÷ (4 × workers).

## Fairness

Corrected relative to the first receipts (RGB-only dump, Stylo sharing off, CC dropping the base sheet, fake-parallel inherit):

- **Same dump.** Eleven properties, including unexercised initials (`position`, `width`, `font-weight`, `visibility`, `color`). Base sheet is cascaded on both sides. `cmp` is the gate.
- **Sharing on.** Stylo’s 32-entry LRU (sibling + cousin after revalidation). CC exact same-parent identity plus the sibling-rule signature (12 302 canons of 20 002 on default, 13 890 on structural, 14 447 on sibling). Share is all-or-nothing on both sides; no property is rematched per node.
- **Inherit is a parent hop.** Specified match stays `@parallel for` over canons. Inherited props copy in one serial depth walk — not one `@parallel for` per level.
- **Same mutation script, same clock.** Stylo takes element snapshots and runs the crate invalidator inside `TIME_MUT_MS`. CC logs the changes at mutate and, inside the same clock, rematches a descendant or following sibling only if its rule signature flips, batch applied vs batch undone, over rules keyed by the changed atoms. Undo / redo is positional so the applied state is exactly what Stylo’s host sees (first duplicate attribute replaced, `classes[0]` removed).
- **Competitor is Stylo the crate**, not the `TElement` host (`host.rs` is glue, like the CC fixture load).

Still not the same program:

- Stylo’s proto is `Arc` style structs (atomic refcount). Ours is a pointer to initial / parent / this node’s slot. Same inherit-vs-reset cut; we do not pay atomics.
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
make compare          # tiny + tiny-sibling + tiny-structural + fixtures/local: both runners, cmp styles
make compare-local    # only fixtures/local (no StyleBench regenerate)
make compare-sibling  # generated tiny sibling combinators vs Stylo
make compare-structural # generated tiny structural pseudo-classes vs Stylo
make bench-style      # default 20k/5k + mutations, release Stylo + CC -O, cmp then times
make bench-sibling    # sibling 20k/5k, release Stylo + warm CC -O, cmp then times
make bench-structural # structural 20k/5k, same shape
```

`CC_STYLE_WORKERS=n` caps the CC worker pool (default: all cores).

`make test` is **upstream Stylo crate tests** (`cargo test --workspace` in `stylo/`). We did not write those. The race fixtures are a frozen [WebKit StyleBench](https://perftest.netlify.app/stylebench/) port (`harness/` = `stylebench-gen`, same LCG / seeds). Stylo is the other runner, not the source of tiny/default.

Add more CSS locally by dropping a `.stylebench` file under `fixtures/local/` (same section markers; both runners already eat it). Do not change tiny/default seeds. `fixtures/*.stylebench` is gitignored; `fixtures/local/` is not.
