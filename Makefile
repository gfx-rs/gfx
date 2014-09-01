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

.PHONY: clean
clean:
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
