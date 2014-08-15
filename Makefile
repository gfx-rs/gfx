all:
	cargo build
	cargo test
	make -C src/tests
	make -C src/examples
	cargo doc

.PHONY: all
