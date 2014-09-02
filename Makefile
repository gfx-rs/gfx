DIAGRAMS_IN_DOT = $(wildcard diagrams/dot/*.dot)
DIAGRAMS_OUT_PNG = $(patsubst diagrams/dot/%.dot,diagrams/png/%.png,$(DIAGRAMS_IN_DOT))

.PHONY: all
all:
	cargo build

.PHONY: update
update:
	(cd src/device && cargo update)
	(cd src/render && cargo update)
	(cd src/gfx_macros && cargo update)
	cargo update
	make -C examples update
	rm -rf doc

.PHONY: test
test:
	(cd src/device && cargo test)
	(cd src/render && cargo test)
	(cd src/gfx_macros && cargo test)
	cargo test

.PHONY: doc
doc:
	cargo doc

$(DIAGRAMS_OUT_PNG): $(DIAGRAMS_IN_DOT)
	@mkdir -p diagrams/png
	dot -Tpng -o $@ -Gsize=6,6 -Gdpi=100 $<

.PHONY: diagrams
diagrams: $(DIAGRAMS_OUT_PNG)

.PHONY: clean-diagrams
clean-diagrams:
	rm -rf diagrams/png

.PHONY: clean
clean: clean-diagrams
	(cd src/device && cargo clean)
	(cd src/render && cargo clean)
	(cd src/gfx_macros && cargo clean)
	cargo clean
	make -C examples clean
	rm -rf doc

.PHONY: travis
travis: test doc
	make -C examples
	# the doc directory needs to be in the root for rust-ci
	mv target/doc doc
