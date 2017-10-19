RUST_BACKTRACE:=1
EXCLUDES:=
FEATURES_WARDEN:=
FEATURES_RENDER:=
FEATURES_RENDER_ADD:= mint serialize
FEATURES_QUAD:=
FEATURES_QUAD2:=
CMD_QUAD_RENDER:=cargo check

SDL2_DEST=$(HOME)/deps
SDL2_CONFIG=$(SDL2_DEST)/usr/bin/sdl2-config
SDL2_PPA=http://ppa.launchpad.net/zoogie/sdl2-snapshots/ubuntu/pool/main/libs/libsdl2


ifeq ($(OS),Windows_NT)
	EXCLUDES+= --exclude gfx-backend-metal
	FEATURES_QUAD=vulkan
	FEATURES_WARDEN+=vulkan
	ifeq ($(TARGET),x86_64-pc-windows-gnu)
		# No d3d12 support on GNU windows ATM
		# context: https://github.com/gfx-rs/gfx/pull/1417
		EXCLUDES+= --exclude gfx-backend-dx12
	else
		FEATURES_QUAD2=dx12
		FEATURES_WARDEN+=dx12
	endif
else
	UNAME_S:=$(shell uname -s)
	EXCLUDES+= --exclude gfx-backend-dx12
	GLUTIN_HEADLESS_FEATURE="--features headless" #TODO?
	ifeq ($(UNAME_S),Linux)
		EXCLUDES+= --exclude gfx-backend-metal
		FEATURES_QUAD=vulkan
		FEATURES_WARDEN+=vulkan
	endif
	ifeq ($(UNAME_S),Darwin)
		EXCLUDES+= --exclude gfx-backend-vulkan
		EXCLUDES+= --exclude quad-render
		FEATURES_QUAD=metal
		FEATURES_WARDEN+=metal
		CMD_QUAD_RENDER=pwd
	endif
endif


.PHONY: all check ex-hal-quad warden reftests render ex-render-quad travis-sdl2

all: check ex-hal-quad warden render ex-render-quad

check:
	cargo check --all $(EXCLUDES)
	cargo test --all $(EXCLUDES)

warden:
	cd src/warden && cargo test

reftests: warden
	cd src/warden && cargo run --bin reftest --features "$(FEATURES_WARDEN)"

render:
	cd src/render && cargo test --features "$(FEATURES_RENDER)"
	cd src/render && cargo test --features "$(FEATURES_RENDER) $(FEATURES_RENDER_ADD)"

ex-hal-quad:
	cd examples/hal/quad && cargo check --features "gl"
	cd examples/hal/quad && cargo check --features "$(FEATURES_QUAD2)"
	cd examples/hal/quad && cargo check --features "$(FEATURES_QUAD)"

ex-render-quad:
	cd examples/render/quad_render && $(CMD_QUAD_RENDER)

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
