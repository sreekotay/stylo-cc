# Friction log

What ports and completeness slices surfaced. Gaps are data; none are
smoothed over in the specimen.

## calc() on lengths (2026-09-04)

A specified length is a linear combination of four bases (`px`, `em`,
`rem`, `%`). `calc(8px + 8px)` and `calc(2em)` fold back onto the
plain cell; mixed basis (`2em + 4px`, `50% + 8px`) is the new fact.
Gated by `fixtures/local/calc.stylebench` against Stylo.

What worked without friction: the 4-tuple compiled first try and
passed `cmp` on fold, `*`, nested `calc()`, `em` / `rem`, leftover
`%` (`calc(50% + 8px)`), inherit of `font-size: calc(2em)`, number
`opacity: calc(1 - 0.25)`, and invalid drop (`8px * 8px`, `8px + 2`,
truncated expr). Stylo's leftover `ToCss` spacing (`" + "` / `" - "`,
`%` before `px`) matched without a second guess.

Not in this slice — still the completeness gap: `var()` inside calc,
`min` / `max` / `clamp`, color calc, `deg` / `s`.

## :is / :not / :where (2026-09-04)

A compound may hold selector-list facts. Alternatives of one `:is()` /
`:not()` / `:where()` are OR; several lists on the same compound are
AND. `:is` / `:not` take the max specificity of the alts; `:where` is
zero. Bloom does not AND those atoms. Gated by
`fixtures/local/is-not.stylebench` against Stylo.

`skip` after a complement class in the balanced-paren production ate
the closing `)` on odd-length args (`:not(.skip)`), so the sheet
truncated after the first two `:is()` rules. `parg_atom` without
`skip` parsed first try after that. Stylo loads `---base---` and
`---css---` as one author sheet, so a subject `:is(.a)` cannot beat
`#testroot *` on `height` — the fixture uses longhands the base does
not set.

What this slice still is not: a forgiving selector list (one bad alt
drops the rule, matching `:not` and being stricter than `:is` /
`:where`), `:has`, `:nth-*(… of S)`, quoted `)` inside an arg.

## var() / custom properties (2026-09-04)

A custom property is inherited token text. `var()` / `var(--n, fb)`
substitutes in `inherit_one` (serial, parent first) so parallel
cascade never reads another node's vars. A longhand whose specified
text still has `var()` is pending until that walk; `calc(var(--x)+4px)`
is the existing calc parser after substitution. Gated by
`fixtures/local/var.stylebench` against Stylo.

StyleBench has no `--*` and no `var()` — `has_vars` stays off.

`prop` must stay a single `keep` (`keep [ident | custom_prop]`). A
choice of two keeps made `fill_decls` miss every longhand name and
dropped the calc fixture.

## box shorthands (2026-09-04)

1–4 whitespace-separated tokens assign onto the existing TRBL
longhands (`margin` / `padding` / `border-width` / `border-style` /
`border-color` / `inset`). 1–2 tokens do the same for a pair
(`overflow` / `gap`). `border` classifies each token as width / style /
color and writes all four sides, then resets `border-image-*`.
`thin` / `medium` / `thick` become 1 / 3 / 5 px so the dump matches
Stylo's computed width. A leftover token drops the whole shorthand.
Gated by `fixtures/local/box.stylebench` against Stylo.

The token splitter is balanced-paren so `calc()` / `var()` / `rgb()`
stay one component — the old `background` walk only special-cased
`rgb()`. Atom longhands accept any text, so `overflow: var(--x)` was
stored as the literal until `take_longhand` parks any `var()` as
pending — length and color cells already failed the parser and took
that path.

Not in this slice: `font`, logicals, grid, `font-family` stacks,
`transform` functions, a `var()` that is the entire `border` value
(cannot tell which cell), `overflow: visible` mixed with a non-visible
axis (used-value `auto`).
