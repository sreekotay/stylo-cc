# stylo-cc

A Concurrent-C styling engine raced against [Stylo](https://github.com/servo/stylo) on a frozen [StyleBench](https://perftest.netlify.app/stylebench/) workload.

```
fixtures/           generated StyleBench races (tiny / default; gitignored)
fixtures/local/     hand CSS we own (em, hex, !important, …) — still cmp vs Stylo
stylo-runner/       real Stylo on a TElement host
engine/             idiomatic Concurrent-C engine
stylo/              git submodule — servo/stylo (oracle crate + unit tests)
harness/            stylebench-gen — frozen WebKit StyleBench LCG / seeds
```

Style only. No layout. No paint. No browser.

The gate is `cmp` of the style dump for every live element. Timings are wall clock after receipts match.

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

## Properties

**Cascaded and dumped** (fixture + a few unexercised initials, so the dump is not only `background-color`):

| property | inherit? | notes |
|---|---|---|
| `display` | no | HTML `div` → `block`, else `inline`; base sheet `inline-block` |
| `position` | no | initial `static` |
| `width` | no | initial `auto` |
| `height` | no | initial `auto`; base sheet `10px` |
| `min-width` | no | initial `auto`; base sheet `10px` |
| `font-size` | yes | initial `16px`; `#testroot` `10px` |
| `line-height` | yes | initial `normal`; `#testroot` `10px` |
| `font-weight` | yes | initial `400` |
| `visibility` | yes | initial `visible` |
| `color` | yes | initial `rgb(0, 0, 0)` |
| `background-color` | no | generated sheet; dumped as `r,g,b,a` |

Selectors: type, `#id`, `.class`, `[attr]` / `[attr=val]`, `*`, descendant, child.

**Not in this engine.** The rest of CSS. In particular:

- **Selectors:** sibling (`+` / `~`), `:nth-*`, `:not` / `:is` / `:where`, `:hover` and other user-action, `::before` / `::after`, media / supports, shadow, namespaces.
- **Properties:** margin, padding, border, flex, grid, transform, animation, `font-family` / `font-style`, text-*, overflow, z-index, opacity, box-sizing, white-space, vertical-align, max-*, min-height, insets, float, …
- **Values:** `rem`, `calc()`, `var()`, hsl, `%` on box lengths. Integer `em` / `%` on `font-size`, `#rgb` / `#rrggbb`, and `currentColor` on `background-color` are in; see `fixtures/local/` (`make compare-local`).

Stylo does not walk every longhand. Unused properties stay on a **proto** (parent `Arc` if inherited, initial `Arc` if reset) and are only COW’d when a declaration hits that struct. StyleBench dirties Box + Background on almost every node; Font is specified on `#testroot` and borrowed by kids. We use the same cut: `StyBg*` / `StyBox*` / `StyFont*` — initial or parent proto, unique slot on first specified write (no memcpy of the proto). The leftover tilt is Stylo’s `Arc` bag (and rule tree), not flex/grid we skipped.

## Recorded times

Default suite, release, wall clock, Apple M5, 2026-08-29. Live after mutations: 20 002. Sharing on both sides. Inherit on CC is serial.

| | first restyle | 25 mutation restyles |
|---|---|---|
| Stylo (6 threads, sharing LRU on) | 30.7 ms | 673 ms |
| Stylo (6 threads, sharing off) | 45.3 ms | 1099 ms |
| CC (`ccc -O`, sibling canons + used protos) | 19.3 ms | 301 ms |

See `receipts/default_2026_08_29.txt`. Tiny (81/81) is `make compare` (also runs `fixtures/local/`); that target builds Stylo debug, so its `TIME_*` lines are not the recorded numbers.

## Fairness

Corrected relative to the first receipts (RGB-only dump, Stylo sharing off, CC dropping the base sheet, fake-parallel inherit):

- **Same dump.** Eleven properties, including unexercised initials (`position`, `width`, `font-weight`, `visibility`, `color`). Base sheet is cascaded on both sides. `cmp` is the gate.
- **Sharing on.** Stylo’s 32-entry LRU (sibling + cousin after revalidation). CC exact same-parent identity (12 854 canons of 20 002 on default).
- **Inherit is a parent hop.** Specified match stays `@parallel for` over canons. Inherited props copy in one serial depth walk — not one `@parallel for` per level.
- **Same mutation script.** Full rematch after each batch. No incremental invalidation on either side.

Still not the same program (Stylo the engine, not our host):

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
make compare          # tiny + local cascade: both runners, cmp styles
make compare-local    # only fixtures/local (no StyleBench regenerate)
make bench-style      # default 20k/5k + mutations, release Stylo + CC -O, cmp then times
```

`make test` is **upstream Stylo crate tests** (`cargo test --workspace` in `stylo/`). We did not write those. The race fixtures are a frozen [WebKit StyleBench](https://perftest.netlify.app/stylebench/) port (`harness/` = `stylebench-gen`, same LCG / seeds). Stylo is the other runner, not the source of tiny/default.

Add more CSS locally by dropping a `.stylebench` file under `fixtures/local/` (same `---base---` / `---css---` / `---tree---` / `---mut---` text; both runners already eat it). `make compare-local` loops that directory. Do not change tiny/default seeds — that is the race. `fixtures/*.stylebench` is gitignored so generated races stay out of git; `fixtures/local/` is not.
