RUST_BACKTRACE:=1
EXCLUDES:=--exclude gfx-backend-webgpu
FEATURES_GL:=
FEATURES_HAL:=
FEATURES_HAL2:=
METAL_SHADERS=src/backend/metal/shaders

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
else
	UNAME_S:=$(shell uname -s)
	EXCLUDES+= --exclude gfx-backend-dx12
	EXCLUDES+= --exclude gfx-backend-dx11
	ifeq ($(UNAME_S),Linux)
		EXCLUDES+= --exclude gfx-backend-metal
		FEATURES_HAL=vulkan
	endif
	ifeq ($(TARGET),aarch64-apple-ios)
		EXCLUDES+= --exclude gfx-backend-vulkan --exclude gfx-backend-gl
	else ifeq ($(TARGET),x86_64-apple-ios)
		EXCLUDES+= --exclude gfx-backend-vulkan --exclude gfx-backend-gl
	else
		FEATURES_GL=gl
	endif
	ifeq ($(UNAME_S),Darwin)
		FEATURES_HAL=metal
	endif
endif


.PHONY: all check check-backends check-wasm quad quad-wasm test doc reftests benches shader-binaries

all: check test

help:
	@echo "Supported backends: $(FEATURES_GL) $(FEATURES_HAL) $(FEATURES_HAL2)"

check: check-backends
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "$(FEATURES_GL)"
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "$(FEATURES_HAL)"
	cd examples && cargo check $(CHECK_TARGET_FLAG) --features "$(FEATURES_HAL2)"
	cd src/warden && cargo check $(CHECK_TARGET_FLAG) --no-default-features
	cd src/warden && cargo check $(CHECK_TARGET_FLAG) --features "env_logger $(FEATURES_GL) $(FEATURES_HAL) $(FEATURES_HAL2)"

check-backends:
	cargo check --all $(CHECK_TARGET_FLAG) $(EXCLUDES) --exclude gfx-warden

check-wasm:
	cd src/backend/webgpu && RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo check --target wasm32-unknown-unknown

test:
	cargo test --all $(EXCLUDES)

doc:
	cargo doc --all $(EXCLUDES)

reftests:
	cd src/warden && cargo run --bin reftest --features "$(FEATURES_GL) $(FEATURES_HAL) $(FEATURES_HAL2)" -- local

benches:
	cd src/warden && cargo run --release --bin bench --features "$(FEATURES_GL) $(FEATURES_HAL) $(FEATURES_HAL2)" -- blit

reftests-ci:
	cd src/warden && cargo test
	cd src/warden && cargo run --features "gl" -- ci

quad:
	cd examples && cargo run --bin quad --features ${FEATURES_HAL}

quad-wasm:
	cd examples && cargo +nightly build --features gl --target wasm32-unknown-unknown --bin quad && wasm-bindgen ../target/wasm32-unknown-unknown/debug/quad.wasm --out-dir ../examples/generated-wasm --web

shader-binaries: $(METAL_SHADERS)/*.metal
ifeq ($(UNAME_S),Darwin)
	# MacOS
	xcrun -sdk macosx metal -c $(METAL_SHADERS)/*.metal -mmacosx-version-min=10.11 -g -MO
	xcrun -sdk macosx metallib *.air -o $(METAL_SHADERS)/gfx-shaders-macos.metallib
	rm *.air
	# iOS
	xcrun -sdk iphoneos metal -c $(METAL_SHADERS)/*.metal -mios-version-min=11.4 -g -MO
	xcrun -sdk iphoneos metallib *.air -o $(METAL_SHADERS)/gfx-shaders-ios.metallib
	rm *.air
	# iOS Simulator
	xcrun -sdk iphonesimulator metal -c $(METAL_SHADERS)/*.metal -mios-simulator-version-min=13.0 -g -MO
	xcrun -sdk iphonesimulator metallib *.air -o $(METAL_SHADERS)/gfx-shaders-ios-simulator.metallib
	rm *.air
endif
