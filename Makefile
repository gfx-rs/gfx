RUST_BACKTRACE:=1
EXCLUDES:=
FEATURES_RENDER:=
FEATURES_RENDER_ADD:= mint serialize
FEATURES_QUAD:=
FEATURES_QUAD_ADD:=
FEATURES_QUAD2:=
CMD_QUAD_RENDER:=cargo check

SDL2_DEST=$(HOME)/deps
SDL2_CONFIG=$(SDL2_DEST)/usr/bin/sdl2-config
SDL2_PPA=http://ppa.launchpad.net/zoogie/sdl2-snapshots/ubuntu/pool/main/libs/libsdl2


ifeq ($(OS),Windows_NT)
	EXCLUDES+= --exclude gfx_backend_metal
	FEATURES_QUAD=vulkan
	ifeq ($(TARGET),x86_64-pc-windows-gnu)
		# No d3d12 support on GNU windows ATM
		# context: https://github.com/gfx-rs/gfx/pull/1417
		EXCLUDES+= --exclude gfx_backend_dx12
	else
		FEATURES_QUAD2=dx12
	endif
else
	UNAME_S:=$(shell uname -s)
	EXCLUDES+= --exclude gfx_device_dx11
	EXCLUDES+= --exclude gfx_backend_dx12
	GLUTIN_HEADLESS_FEATURE="--features headless" #TODO?
	ifeq ($(UNAME_S),Linux)
		EXCLUDES+= --exclude gfx_backend_metal
		FEATURES_QUAD=vulkan
	endif
	ifeq ($(UNAME_S),Darwin)
		EXCLUDES+= --exclude gfx_backend_vulkan
		EXCLUDES+= --exclude quad_render
		FEATURES_QUAD=metal
		FEATURES_QUAD_ADD=metal_argument_buffer
		CMD_QUAD_RENDER=pwd
	endif
endif


.PHONY: all check render ex-quad travis-sdl2

all: check render ex-quad ex-quad-render

check:
	cargo check --all $(EXCLUDES)
	cargo test --all $(EXCLUDES)

render:
	cd src/render && cargo test --features "$(FEATURES_RENDER)"
	cd src/render && cargo test --features "$(FEATURES_RENDER) $(FEATURES_RENDER_ADD)"

ex-quad:
	cd examples/core/quad && cargo check --features "gl"
	cd examples/core/quad && cargo check --features "$(FEATURES_QUAD2)"
	cd examples/core/quad && cargo check --features "$(FEATURES_QUAD)"
	cd examples/core/quad && cargo check --features "$(FEATURES_QUAD) $(FEATURES_QUAD_ADD)"

ex-quad-render:
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
