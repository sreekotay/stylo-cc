# StyleBench fixture + two runners. Upstream crate tests stay under stylo/.

STYLO ?= stylo
CARGO ?= cargo
RECEIPTS ?= receipts
FIXTURES ?= fixtures
CCC ?= ccc

.PHONY: setup fixture fixture-default stylo-run cc-run compare compare-local bench-style test build release bench help

help:
	@echo "fixture         tiny suite (81 / 40) for the compile loop"
	@echo "fixture-default StyleBench default (20k / 5k)"
	@echo "compare         tiny + fixtures/local: cmp styles"
	@echo "compare-local   every fixtures/local/*.stylebench vs Stylo"
	@echo "bench-style     default suite, release Stylo + CC -O, cmp then times"
	@echo "test            upstream cargo test --workspace"

setup:
	git submodule update --init --depth 1

fixture: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen -- --tiny > $(FIXTURES)/tiny.stylebench

stylo-run: fixture
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $(FIXTURES)/tiny.stylebench > $(RECEIPTS)/tiny.stylo.txt

cc-run: fixture
	mkdir -p $(RECEIPTS)
	$(CCC) run engine/stylebench_cc.ccs -- $(FIXTURES)/tiny.stylebench > $(RECEIPTS)/tiny.cc.txt

fixture-default: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen --release > $(FIXTURES)/default.stylebench

compare: stylo-run cc-run compare-local
	@grep -v '^#' $(RECEIPTS)/tiny.stylo.txt > /tmp/stylo.styles
	@grep -v '^#' $(RECEIPTS)/tiny.cc.txt > /tmp/cc.styles
	cmp /tmp/stylo.styles /tmp/cc.styles
	@echo OK tiny
	@grep '^# TIME' $(RECEIPTS)/tiny.stylo.txt $(RECEIPTS)/tiny.cc.txt

# Hand-written CSS (not the StyleBench generator). Same dump / cmp vs Stylo.
# Add a .stylebench under fixtures/local/; they are not gitignored (tiny/default are).
LOCAL_FIXTURES := $(sort $(wildcard $(FIXTURES)/local/*.stylebench))
compare-local:
	mkdir -p $(RECEIPTS)
	@set -e; for f in $(LOCAL_FIXTURES); do \
		name=$$(basename $$f .stylebench); \
		echo "== $$name =="; \
		$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $$f > $(RECEIPTS)/$$name.stylo.txt; \
		$(CCC) run engine/stylebench_cc.ccs -- $$f > $(RECEIPTS)/$$name.cc.txt; \
		grep -v '^#' $(RECEIPTS)/$$name.stylo.txt > /tmp/stylo.local; \
		grep -v '^#' $(RECEIPTS)/$$name.cc.txt > /tmp/cc.local; \
		cmp /tmp/stylo.local /tmp/cc.local; \
		echo OK $$name; \
	done

# First restyle + StyleBench mutation steps (class / attr / leaf), then cmp.
bench-style: fixture-default
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --release --manifest-path stylo-runner/Cargo.toml -- \
		$(FIXTURES)/default.stylebench > $(RECEIPTS)/default.stylo.txt
	$(CCC) build run -O engine/stylebench_cc.ccs -- \
		$(FIXTURES)/default.stylebench > $(RECEIPTS)/default.cc.txt
	@grep -v '^#' $(RECEIPTS)/default.stylo.txt > /tmp/stylo.styles
	@grep -v '^#' $(RECEIPTS)/default.cc.txt > /tmp/cc.styles
	cmp /tmp/stylo.styles /tmp/cc.styles
	@echo OK
	@grep '^# TIME' $(RECEIPTS)/default.stylo.txt $(RECEIPTS)/default.cc.txt

build: $(STYLO)/Cargo.toml
	cd $(STYLO) && $(CARGO) build --workspace

test: $(STYLO)/Cargo.toml
	cd $(STYLO) && $(CARGO) test --workspace

release: $(STYLO)/Cargo.toml
	cd $(STYLO) && $(CARGO) build --release --features servo

bench: $(STYLO)/Cargo.toml
	./scripts/upstream.sh bench

$(STYLO)/Cargo.toml:
	@echo "stylo/ missing — run: git submodule update --init --depth 1" >&2
	@exit 1
