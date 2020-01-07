RUST_BACKTRACE:=1
EXCLUDES:=
FEATURES_HAL:=
FEATURES_HAL2:=
FEATURES_HAL3:=

ifeq (,$(TARGET))
	CHECK_TARGET_FLAG=
else
	CHECK_TARGET_FLAG=--target $(TARGET)
endif

ifeq ($(OS),Windows_NT)
	EXCLUDES+= --exclude gfx-backend-metal
	FEATURES_HAL=vulkan
	ifeq ($(TARGET),x86_64-pc-windows-gnu)
		# No d3d12 support on GNU windows ATM
		# context: https://github.com/gfx-rs/gfx/pull/1417
		EXCLUDES+= --exclude gfx-backend-dx12
		EXCLUDES+= --exclude gfx-backend-dx11
	else
		FEATURES_HAL2=dx12
	endif
	FEATURES_HAL3=wgl
else
	UNAME_S:=$(shell uname -s)
	EXCLUDES+= --exclude gfx-backend-dx12
	EXCLUDES+= --exclude gfx-backend-dx11
	ifeq ($(UNAME_S),Linux)
		EXCLUDES+= --exclude gfx-backend-metal
		FEATURES_HAL=vulkan
	endif
	ifeq ($(TARGET),aarch64-apple-ios)
		EXCLUDES+= --exclude gfx-backend-vulkan
	endif
	ifeq ($(UNAME_S),Darwin)
		FEATURES_HAL=metal
	endif
endif


.PHONY: all check quad quad-wasm test doc reftests benches shader-binaries

all: check test

help:
	@echo "Supported backends: gl $(FEATURES_HAL) $(FEATURES_HAL2)"

check:
	@echo "Note: excluding \`warden\` here, since it depends on serialization"
	cargo check --all $(CHECK_TARGET_FLAG) $(EXCLUDES) --exclude gfx-warden
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "gl"
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "$(FEATURES_HAL)"
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "$(FEATURES_HAL2)"
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "$(FEATURES_HAL3)"
	cd src/warden && cargo check $(CHECK_TARGET_FLAG) --no-default-features
	cd src/warden && cargo check $(CHECK_TARGET_FLAG) --features "env_logger gl gl-ci $(FEATURES_HAL) $(FEATURES_HAL2)"

test:
	cargo test --all $(EXCLUDES)

doc:
	cargo doc --all $(EXCLUDES)

reftests:
	cd src/warden && cargo run --bin reftest --features "$(FEATURES_HAL) $(FEATURES_HAL2)" -- local #TODO: gl

benches:
	cd src/warden && cargo run --release --bin bench --features "$(FEATURES_HAL) $(FEATURES_HAL2)" -- blit

reftests-ci:
	cd src/warden && cargo test
	cd src/warden && cargo run --features "gl-ci" -- ci

quad:
	cd examples && cargo run --bin quad --features ${FEATURES_HAL}

quad-wasm:
	cd examples && cargo +nightly build --target wasm32-unknown-unknown --features gl --bin quad && wasm-bindgen ../target/wasm32-unknown-unknown/debug/quad.wasm --out-dir ../examples/generated-wasm --web

shader-binaries:
ifeq ($(UNAME_S),Darwin)
	# MacOS
	cd ./src/backend/metal/shaders && \
	xcrun -sdk macosx metal -c *.metal -mmacosx-version-min=10.11 && \
	xcrun -sdk macosx metallib *.air -o gfx-shaders-macos.metallib && \
	rm *.air
	# iOS
	cd ./src/backend/metal/shaders && \
	xcrun -sdk iphoneos metal -c *.metal -mios-version-min=11.4 && \
	xcrun -sdk iphoneos metallib *.air -o gfx-shaders-ios.metallib && \
	rm *.air
endif
