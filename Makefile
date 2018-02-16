RUST_BACKTRACE:=1
EXCLUDES:=
FEATURES_RENDER:=
FEATURES_EXTRA:=mint serialize
FEATURES_HAL:=
FEATURES_HAL2:=
FEATURES_WARDEN:=gl
CMD_QUAD_RENDER:=cargo check

SDL2_DEST=$(HOME)/deps
SDL2_CONFIG=$(SDL2_DEST)/usr/bin/sdl2-config
SDL2_PPA=http://ppa.launchpad.net/zoogie/sdl2-snapshots/ubuntu/pool/main/libs/libsdl2


ifeq ($(OS),Windows_NT)
	EXCLUDES+= --exclude gfx-backend-metal
	FEATURES_HAL=vulkan
	ifeq ($(TARGET),x86_64-pc-windows-gnu)
		# No d3d12 support on GNU windows ATM
		# context: https://github.com/gfx-rs/gfx/pull/1417
		EXCLUDES+= --exclude gfx-backend-dx12
	else
		FEATURES_HAL2=dx12
	endif
else
	UNAME_S:=$(shell uname -s)
	EXCLUDES+= --exclude gfx-backend-dx12
	GLUTIN_HEADLESS_FEATURE="--features headless" #TODO?
	ifeq ($(UNAME_S),Linux)
		EXCLUDES+= --exclude gfx-backend-metal
		FEATURES_HAL=vulkan
		FEATURES_WARDEN+= glsl-to-spirv
	endif
	ifeq ($(UNAME_S),Darwin)
		EXCLUDES+= --exclude gfx-backend-vulkan
		EXCLUDES+= --exclude quad-render
		FEATURES_HAL=metal
		CMD_QUAD_RENDER=pwd
	endif
endif


.PHONY: all check test reftests travis-sdl2

all: check test

help:
	@echo "Supported backends: gl $(FEATURES_HAL) $(FEATURES_HAL2)"

check:
	#Note: excluding `warden` here, since it depends on serialization
	cargo check --all $(EXCLUDES) --exclude gfx-warden
	cd examples/hal && cargo check --features "gl"
	cd examples/hal && cargo check --features "$(FEATURES_HAL)"
	cd examples/hal && cargo check --features "$(FEATURES_HAL2)"
	cd examples/render/quad_render && $(CMD_QUAD_RENDER)
	cd src/warden && cargo check --features "env_logger gl gl-headless $(FEATURES_HAL) $(FEATURES_HAL2)"

test:
	cargo test --all $(EXCLUDES)
	cd src/render && cargo test --features "$(FEATURES_RENDER) $(FEATURES_EXTRA)"

reftests:
	cd src/warden && cargo test --features "$(FEATURES_WARDEN)"
	cd src/warden && cargo run --features "$(FEATURES_WARDEN) $(FEATURES_HAL) $(FEATURES_HAL2)" -- local

reftests-ci:
	cd src/warden && cargo run --features "$(FEATURES_WARDEN)" -- ci #TODO: "gl-headless"

travis-sdl2:
	#TODO
	#if [ -e $(SDL2_CONFIG) ]; then exit 1; fi
	#mkdir -p $(SDL2_DEST)
	#test -f $(SDL2_DEST)/dev.deb || curl -sLo $(SDL2_DEST)/dev.deb $(SDL2_PPA)/libsdl2-dev_2.0.3+z4~20140315-8621-1ppa1precise1_amd64.deb
	#test -f $(SDL2_DEST)/bin.deb || curl -sLo $(SDL2_DEST)/bin.deb $(SDL2_PPA)/libsdl2_2.0.3+z4~20140315-8621-1ppa1precise1_amd64.deb
	#dpkg-deb -x $(SDL2_DEST)/bin.deb .
	#dpkg-deb -x $(SDL2_DEST)/dev.deb .
	#sed -e s,/usr,$(SDL2_DEST),g $(SDL2_CONFIG) > $(SDL2_CONFIG).new
	#mv $(SDL2_CONFIG).new $(SDL2_CONFIG)
	#chmod a+x $(SDL2_CONFIG)
