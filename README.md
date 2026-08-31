# stylo-cc

A Concurrent-C styling engine raced against [Stylo](https://github.com/servo/stylo) on a frozen [StyleBench](https://perftest.netlify.app/stylebench/) workload.

```
fixtures/           generated StyleBench races (tiny / default; gitignored)
fixtures/local/     hand CSS we own — still cmp vs Stylo
stylo-runner/       real Stylo (`style` crate) on a TElement host
engine/             idiomatic Concurrent-C engine
stylo/              git submodule — servo/stylo 0.20.0 (oracle crate + unit tests)
harness/            stylebench-gen — frozen WebKit StyleBench LCG / seeds
```

Style only. No layout. No paint. No browser. Stylo submodule tracks `origin/main` (`b3e6425`).

The gate is `cmp` of the style dump for every live element. Timings are wall clock after receipts match.

This is a StyleBench race against Stylo the crate, not a Firefox hook. A win here is not “faster Firefox.”

## What is being tested

Both runners eat the same text fixture and must print the same style dump.

**Sheet.** StyleBench `defaultConfiguration`: 5 000 rules. Combinators are descendant (` `) and child (`>`) only. Each generated rule sets `background-color: rgb(...)`. The base sheet sets `#testroot` font-size / line-height and `#testroot *` display / height / min-width. No sibling combinators, no `:nth-*`, no `::before` / `::after`, no media queries.

**Tree.** Same LCG as WebKit (`styleSeed=1`, `domSeed=2`). Default is 20 000 generated elements plus `#testroot` (20 001 nodes). Tags, ids, classes, and attributes are the StyleBench random draw (class pool 200, id chance 0.05, …).

**First restyle (`TIME_MS`).** Match live nodes and cascade the dumped properties. Stylo: `recalc_style_at` via `traverse_dom` on Servo’s `STYLE_THREAD_POOL` (cap 6), 32-entry sharing LRU on. CC: subject / pair inverted maps, ancestor bloom; exact sibling share (same parent + interned tag / id / classes / attrs) then `@parallel for` over canons; inherited properties hop the parent in one serial depth walk.

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

That is 25 restyles on default (5 on tiny). Each restyle still walks the live tree (Stylo sharing LRU; CC sibling canons). Neither side does mutation-local invalidation (Stylo snapshots / restyle hints, or a CC dirty set). `TIME_MUT_MS` is the sum of those restyles only — applying the DOM edits is outside the clock.

**Not measured.** Layout, paint, resize, sibling / structural / nth suites, `::before` / `::after`, incremental invalidation.

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

Selectors: type, `#id`, `.class`, `[attr]` / `[attr=val]`, `*`, descendant, child.

**Not in this engine.** The rest of CSS. In particular:

- **Selectors:** sibling (`+` / `~`), `:nth-*`, `:not` / `:is` / `:where`, `:hover` and other user-action, `::before` / `::after`, media / supports, shadow, namespaces.
- **Properties:** margin, padding, border, flex, grid, transform, animation, `font-family` / `font-style`, text-*, overflow, z-index, opacity, box-sizing, white-space, vertical-align, max-*, min-height, insets, float, …
- **Values:** `calc()`, `var()`, hsl, `%` on box lengths. Named colors beyond `transparent` / `currentColor`.

Stylo does not walk every longhand. Unused properties stay on a **proto** (parent `Arc` if inherited, initial `Arc` if reset) and are only COW’d when a declaration hits that struct. StyleBench dirties Box + Background on almost every node; Font is specified on `#testroot` and borrowed by kids. We use the same cut: `StyBg*` / `StyBox*` / `StyFont*` — initial or parent proto, unique slot on first specified write (no memcpy of the proto). The leftover tilt is Stylo’s `Arc` bag (and rule tree), not flex/grid we skipped.

## Recorded times

Default suite, release, wall clock, Apple M5, 2026-08-30. Live after mutations: 20 002. Sharing on both sides. Inherit on CC is serial. `ccc` 0.3.4-259 (`-O`). Stylo `0.20.0` (`b3e6425`). `cmp` clean.

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 30.6 ms | 669 ms |
| Stylo (6 threads, sharing off) | 45.3 ms | 1099 ms |
| CC (`ccc -O`, sibling canons + used protos) | 18.3 ms | 313 ms |

Sharing-off is from `receipts/default_2026_08_29.txt` (not re-run). Sharing-on / CC are this rebuild. Tiny (81/81) is `make compare` (also runs `fixtures/local/`); that target builds Stylo debug, so its `TIME_*` lines are not the recorded numbers.

CC split (this run): match ~15.3, last match ~10.9, inherit ~0.12, **12854 canons / 20002**.

## Fairness

Corrected relative to the first receipts (RGB-only dump, Stylo sharing off, CC dropping the base sheet, fake-parallel inherit):

- **Same dump.** Eleven properties, including unexercised initials (`position`, `width`, `font-weight`, `visibility`, `color`). Base sheet is cascaded on both sides. `cmp` is the gate.
- **Sharing on.** Stylo’s 32-entry LRU (sibling + cousin after revalidation). CC exact same-parent identity (12 854 canons of 20 002 on default).
- **Inherit is a parent hop.** Specified match stays `@parallel for` over canons. Inherited props copy in one serial depth walk — not one `@parallel for` per level.
- **Same mutation script.** Full rematch after each batch. No incremental invalidation on either side.
- **Competitor is Stylo the crate**, not the `TElement` host (`host.rs` is glue, like the CC fixture load).

Still not the same program:

- Stylo’s proto is `Arc` style structs (atomic refcount). Ours is a pointer to initial / parent / this node’s slot. Same inherit-vs-reset cut; we do not pay atomics.
- Sharing algorithms differ (LRU + revalidation vs exact sibling key).
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
make compare          # tiny + fixtures/local: both runners, cmp styles
make compare-local    # only fixtures/local (no StyleBench regenerate)
make bench-style      # default 20k/5k + mutations, release Stylo + CC -O, cmp then times
```

`make test` is **upstream Stylo crate tests** (`cargo test --workspace` in `stylo/`). We did not write those. The race fixtures are a frozen [WebKit StyleBench](https://perftest.netlify.app/stylebench/) port (`harness/` = `stylebench-gen`, same LCG / seeds). Stylo is the other runner, not the source of tiny/default.

Add more CSS locally by dropping a `.stylebench` file under `fixtures/local/` (same section markers; both runners already eat it). Do not change tiny/default seeds. `fixtures/*.stylebench` is gitignored; `fixtures/local/` is not.
