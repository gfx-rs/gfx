RUST_BACKTRACE:=1
EXCLUDES:=
FEATURES_EXTRA:=mint serialize
FEATURES_HAL:=
FEATURES_HAL2:=

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
		EXCLUDES+= --exclude gfx-backend-dx11
	else
		FEATURES_HAL2=dx12
	endif
else
	UNAME_S:=$(shell uname -s)
	EXCLUDES+= --exclude gfx-backend-dx12
	EXCLUDES+= --exclude gfx-backend-dx11
	GLUTIN_HEADLESS_FEATURE="--features headless" #TODO?
	ifeq ($(UNAME_S),Linux)
		EXCLUDES+= --exclude gfx-backend-metal
		FEATURES_HAL=vulkan
	endif
	ifeq ($(UNAME_S),Darwin)
		EXCLUDES+= --exclude gfx-backend-vulkan
		FEATURES_HAL=metal
	endif
endif


.PHONY: all check quad test reftests travis-sdl2

all: check test

help:
	@echo "Supported backends: gl $(FEATURES_HAL) $(FEATURES_HAL2)"

check:
	@echo "Note: excluding \`warden\` here, since it depends on serialization"
	cargo check --all $(EXCLUDES) --exclude gfx-warden
	cd examples && cargo check --features "gl"
	cd examples && cargo check --features "$(FEATURES_HAL)"
	cd examples && cargo check --features "$(FEATURES_HAL2)"
	cd src/warden && cargo check --no-default-features
	cd src/warden && cargo check --features "env_logger gl gl-headless $(FEATURES_HAL) $(FEATURES_HAL2)"

test:
	cargo test --all $(EXCLUDES)

reftests:
	cd src/warden && cargo run --features "$(FEATURES_HAL) $(FEATURES_HAL2)" -- local #TODO: gl

reftests-ci:
	cd src/warden && cargo test --features "gl"
	cd src/warden && cargo run --features "gl" -- ci #TODO: "gl-headless"

quad:
	cd examples && cargo run --bin quad --features ${FEATURES_HAL}

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
