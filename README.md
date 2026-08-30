# stylo-cc

A Concurrent-C styling engine raced against [Stylo](https://github.com/servo/stylo) on a frozen [StyleBench](https://perftest.netlify.app/stylebench/) workload.

```
fixtures/       StyleBench trees + sheets (same LCG / seeds as WebKit)
stylo-runner/   real Stylo on a TElement host
engine/         idiomatic Concurrent-C engine
stylo/          git submodule — servo/stylo (oracle crate + unit tests)
```

Style only. No layout. No browser.

## Setup

Rust stable and the `stylo/` submodule. The CC runner needs [`ccc`](https://github.com/sreekotay/concurrent-c).

```bash
source "$HOME/.cargo/env"
git submodule update --init --depth 1
```

## Run

```bash
make fixture          # tiny suite (compile / cmp loop)
make compare          # tiny: both runners, cmp styles
make bench-style      # default 20k/5k, release Stylo + CC -O, cmp then times
```

Upstream Stylo crate tests: `make test` (or `./scripts/upstream.sh test`).

## Workload

The generator is a port of StyleBench’s `Random` + tree/sheet builder (same seeds). First suite is the default descendant/child config, scaled down until both runners are honest, then full 20k/5k.
