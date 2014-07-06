# Copyright 2014 The Gfx-rs Developers.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

RUSTC                 ?= rustc
RUSTDOC               ?= rustdoc

MAKE = make
ifeq ($(OS), Windows_NT)
	MAKE = mingw32-make
endif

LINK_ARGS             = $(shell sh etc/glfw-link-args.sh)

SRC_DIR               = src
DEPS_DIR              = deps
COMM_FILE             = $(SRC_DIR)/comm/lib.rs
DEVICE_FILE           = $(SRC_DIR)/device/lib.rs
PLATFORM_FILE         = $(SRC_DIR)/platform/lib.rs
RENDER_FILE           = $(SRC_DIR)/render/lib.rs
EXAMPLE_FILES         = $(SRC_DIR)/examples/*/*.rs
LIB_FILE              = $(SRC_DIR)/gfx/lib.rs

DOC_DIR               = doc
EXAMPLES_DIR          = examples
LIB_DIR               = lib
DEPS_LIB_DIRS         = $(wildcard $(DEPS_DIR)/*/lib)

DEPS_INCLUDE_FLAGS    = $(patsubst %,-L %, $(DEPS_LIB_DIRS))
LIB_INCLUDE_FLAGS     = -L $(LIB_DIR) $(DEPS_INCLUDE_FLAGS)
EXAMPLE_INCLUDE_FLAGS = -L $(LIB_DIR) $(DEPS_INCLUDE_FLAGS)

GFX_API               ?= gl
GFX_PLATFORM          ?= glfw

DEVICE_CFG            = --cfg=$(GFX_API)
LIB_CFG               = --cfg=$(GFX_PLATFORM)

all: device lib examples doc

submodule-update:
	@git submodule init
	@git submodule update --recursive

$(DEPS_DIR)/gl-rs/README.md: submodule-update

deps: $(DEPS_DIR)/gl-rs/README.md
	$(MAKE) lib -C $(DEPS_DIR)/gl-rs
	$(MAKE) lib -C $(DEPS_DIR)/glfw-rs

libdir:
	mkdir -p $(LIB_DIR)

comm: libdir
	$(RUSTC) --out-dir=$(LIB_DIR) -O $(COMM_FILE)

device: libdir comm
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(DEVICE_CFG) -O $(DEVICE_FILE)

platform: libdir device
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(LIB_CFG) -O $(PLATFORM_FILE)

render: libdir device comm
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(LIB_CFG) -O $(RENDER_FILE)

lib: libdir device platform render
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(LIB_CFG) -O $(LIB_FILE)

doc:
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) $(LIB_INCLUDE_FLAGS) $(GFX_CFG) -o $(DOC_DIR) $(LIB_FILE)

examples-dir:
	mkdir -p $(EXAMPLES_DIR)

$(EXAMPLE_FILES): lib examples-dir
	$(RUSTC) $(EXAMPLE_INCLUDE_FLAGS) --out-dir=$(EXAMPLES_DIR) $@

examples: $(EXAMPLE_FILES)

clean-deps:
	$(MAKE) clean -C $(DEPS_DIR)/gl-rs
	$(MAKE) clean -C $(DEPS_DIR)/glfw-rs

clean:
	rm -rf $(LIB_DIR)
	rm -rf $(EXAMPLES_DIR)

.PHONY: \
	all \
	submodule-update \
	deps \
	device \
	lib \
	doc \
	examples \
	examples-dir \
	$(EXAMPLE_FILES) \
	clean
