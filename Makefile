# StyleBench fixture + two runners. Upstream crate tests stay under stylo/.

STYLO ?= stylo
CARGO ?= cargo
RECEIPTS ?= receipts
FIXTURES ?= fixtures
CCC ?= ccc

.PHONY: setup fixture fixture-default fixture-sibling-tiny fixture-structural-tiny fixture-nth-tiny fixture-ba-tiny fixture-media-tiny stylo-run cc-run compare compare-local compare-sibling compare-structural compare-nth compare-ba compare-media bench-style bench-sibling bench-structural bench-nth bench-ba bench-media test build release bench help

help:
	@echo "fixture         tiny suite (81 / 40) for the compile loop"
	@echo "fixture-default StyleBench default (20k / 5k)"
	@echo "compare         tiny + sibling/structural/nth/ba/media-tiny + fixtures/local: cmp styles"
	@echo "compare-local   every fixtures/local/*.stylebench vs Stylo"
	@echo "compare-sibling generated tiny sibling combinators vs Stylo"
	@echo "compare-structural generated tiny structural pseudo-classes vs Stylo"
	@echo "compare-nth     generated tiny :nth-* pseudo-classes vs Stylo"
	@echo "compare-ba      generated tiny ::before / ::after vs Stylo"
	@echo "compare-media   generated tiny @media + resize steps vs Stylo"
	@echo "bench-style     default suite, release Stylo + CC -O, cmp then times"
	@echo "bench-sibling   sibling 20k/5k, release Stylo + warm CC -O, cmp then times"
	@echo "bench-structural structural 20k/5k, same shape as bench-sibling"
	@echo "bench-nth       nth 20k/5k, same shape as bench-sibling"
	@echo "bench-ba        before/after 20k/5k, same shape as bench-sibling"
	@echo "bench-media     media queries 5k/5k, 55 resizes, same shape as bench-sibling"
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

fixture-sibling-tiny: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen -- --tiny --sibling > $(FIXTURES)/tiny_sibling.stylebench

fixture-structural-tiny: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen -- --tiny --structural > $(FIXTURES)/tiny_structural.stylebench

fixture-nth-tiny: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen -- --tiny --nth > $(FIXTURES)/tiny_nth.stylebench

fixture-ba-tiny: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen -- --tiny-before-after > $(FIXTURES)/tiny_ba.stylebench

fixture-media-tiny: $(STYLO)/Cargo.toml
	mkdir -p $(FIXTURES)
	$(CARGO) run -q -p stylebench-gen -- --tiny-media > $(FIXTURES)/tiny_media.stylebench

compare-sibling: fixture-sibling-tiny
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $(FIXTURES)/tiny_sibling.stylebench > $(RECEIPTS)/tiny_sibling.stylo.txt
	$(CCC) run engine/stylebench_cc.ccs -- $(FIXTURES)/tiny_sibling.stylebench > $(RECEIPTS)/tiny_sibling.cc.txt
	@grep -v '^#' $(RECEIPTS)/tiny_sibling.stylo.txt > /tmp/stylo.sib
	@grep -v '^#' $(RECEIPTS)/tiny_sibling.cc.txt > /tmp/cc.sib
	cmp /tmp/stylo.sib /tmp/cc.sib
	@echo OK tiny-sibling
	@grep '^# TIME' $(RECEIPTS)/tiny_sibling.stylo.txt $(RECEIPTS)/tiny_sibling.cc.txt

compare-structural: fixture-structural-tiny
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $(FIXTURES)/tiny_structural.stylebench > $(RECEIPTS)/tiny_structural.stylo.txt
	$(CCC) run engine/stylebench_cc.ccs -- $(FIXTURES)/tiny_structural.stylebench > $(RECEIPTS)/tiny_structural.cc.txt
	@grep -v '^#' $(RECEIPTS)/tiny_structural.stylo.txt > /tmp/stylo.str
	@grep -v '^#' $(RECEIPTS)/tiny_structural.cc.txt > /tmp/cc.str
	cmp /tmp/stylo.str /tmp/cc.str
	@echo OK tiny-structural
	@grep '^# TIME' $(RECEIPTS)/tiny_structural.stylo.txt $(RECEIPTS)/tiny_structural.cc.txt

compare-nth: fixture-nth-tiny
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $(FIXTURES)/tiny_nth.stylebench > $(RECEIPTS)/tiny_nth.stylo.txt
	$(CCC) run engine/stylebench_cc.ccs -- $(FIXTURES)/tiny_nth.stylebench > $(RECEIPTS)/tiny_nth.cc.txt
	@grep -v '^#' $(RECEIPTS)/tiny_nth.stylo.txt > /tmp/stylo.nth
	@grep -v '^#' $(RECEIPTS)/tiny_nth.cc.txt > /tmp/cc.nth
	cmp /tmp/stylo.nth /tmp/cc.nth
	@echo OK tiny-nth
	@grep '^# TIME' $(RECEIPTS)/tiny_nth.stylo.txt $(RECEIPTS)/tiny_nth.cc.txt

compare-ba: fixture-ba-tiny
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $(FIXTURES)/tiny_ba.stylebench > $(RECEIPTS)/tiny_ba.stylo.txt
	$(CCC) run engine/stylebench_cc.ccs -- $(FIXTURES)/tiny_ba.stylebench > $(RECEIPTS)/tiny_ba.cc.txt
	@grep -v '^#' $(RECEIPTS)/tiny_ba.stylo.txt > /tmp/stylo.ba
	@grep -v '^#' $(RECEIPTS)/tiny_ba.cc.txt > /tmp/cc.ba
	cmp /tmp/stylo.ba /tmp/cc.ba
	@echo OK tiny-ba
	@grep '^# TIME' $(RECEIPTS)/tiny_ba.stylo.txt $(RECEIPTS)/tiny_ba.cc.txt

compare-media: fixture-media-tiny
	mkdir -p $(RECEIPTS)
	$(CARGO) run -q --manifest-path stylo-runner/Cargo.toml -- $(FIXTURES)/tiny_media.stylebench > $(RECEIPTS)/tiny_media.stylo.txt
	$(CCC) run engine/stylebench_cc.ccs -- $(FIXTURES)/tiny_media.stylebench > $(RECEIPTS)/tiny_media.cc.txt
	@grep -v '^#' $(RECEIPTS)/tiny_media.stylo.txt > /tmp/stylo.media
	@grep -v '^#' $(RECEIPTS)/tiny_media.cc.txt > /tmp/cc.media
	cmp /tmp/stylo.media /tmp/cc.media
	@echo OK tiny-media
	@grep '^# TIME' $(RECEIPTS)/tiny_media.stylo.txt $(RECEIPTS)/tiny_media.cc.txt

compare: stylo-run cc-run compare-local compare-sibling compare-structural compare-nth compare-ba compare-media
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

# Same shape as bench-style, one generated suite each.
bench-sibling: $(STYLO)/Cargo.toml
	./scripts/bench-suite.sh sibling

bench-structural: $(STYLO)/Cargo.toml
	./scripts/bench-suite.sh structural

bench-nth: $(STYLO)/Cargo.toml
	./scripts/bench-suite.sh nth

bench-ba: $(STYLO)/Cargo.toml
	./scripts/bench-suite.sh ba

bench-media: $(STYLO)/Cargo.toml
	./scripts/bench-suite.sh media

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
