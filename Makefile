.PHONY: all
all:
	cargo build

.PHONY: update
update:
	(cd src/device && cargo update)
	(cd src/render && cargo update)
	(cd src/gfx_macros && cargo update)
	cargo update
	(cd src/tests && cargo update)
	make -C src/examples update
	rm -rf doc

.PHONY: test
test:
	(cd src/device && cargo test)
	(cd src/render && cargo test)
	(cd src/gfx_macros && cargo test)
	cargo test
	(cd src/tests && cargo test)

.PHONY: doc
doc:
	cargo doc

.PHONY: clean
clean:
	(cd src/device && cargo clean)
	(cd src/render && cargo clean)
	(cd src/gfx_macros && cargo clean)
	cargo clean
	(cd src/tests && cargo clean)
	make -C src/examples clean
	rm -rf doc

.PHONY: travis
travis: test doc
	make -C src/examples
	# the doc directory needs to be in the root for rust-ci
	mv target/doc doc
