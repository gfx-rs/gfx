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
GLFW_PLATFORM_FILE    = $(SRC_DIR)/glfw_platform/lib.rs
RENDER_FILE           = $(SRC_DIR)/render/lib.rs
EXAMPLE_FILES         = $(SRC_DIR)/examples/*/*.rs
LIB_FILE              = $(SRC_DIR)/gfx/lib.rs

COMM_INPUT            = $(SRC_DIR)/comm/*.rs
DEVICE_INPUT          = $(SRC_DIR)/device/*.rs $(SRC_DIR)/device/gl/*.rs
GLFW_PLATFORM_INPUT   = $(SRC_DIR)/glfw_platform/*.rs
RENDER_INPUT          = $(SRC_DIR)/render/*.rs
LIB_INPUT             = $(SRC_DIR)/gfx/*.rs

DOC_DIR               = doc
EXAMPLES_DIR          = examples
LIB_DIR               = lib
TEST_DIR              = test
DEPS_LIB_DIRS         = $(wildcard $(DEPS_DIR)/*/lib)

COMM_OUT              = $(LIB_DIR)/libcomm.rlib
DEVICE_OUT            = $(LIB_DIR)/libdevice.rlib
GLFW_PLATFORM_OUT     = $(LIB_DIR)/libglfw_platform.rlib
RENDER_OUT            = $(LIB_DIR)/librender.rlib
LIB_OUT               = $(LIB_DIR)/libgfx.rlib

COMM_TEST_OUT         = $(TEST_DIR)/comm
DEVICE_TEST_OUT       = $(TEST_DIR)/device
GLFW_PLATFORM_TEST_OUT= $(TEST_DIR)/glfw_platform
RENDER_TEST_OUT       = $(TEST_DIR)/render
LIB_TEST_OUT          = $(TEST_DIR)/gfx

DEPS_INCLUDE_FLAGS    = $(patsubst %,-L %, $(DEPS_LIB_DIRS))
LIB_INCLUDE_FLAGS     = -L $(LIB_DIR) $(DEPS_INCLUDE_FLAGS)
EXAMPLE_INCLUDE_FLAGS = -L $(LIB_DIR) $(DEPS_INCLUDE_FLAGS)

GFX_API               ?= gl
GFX_PLATFORM          ?= glfw

DEVICE_CFG            = --cfg=$(GFX_API)
LIB_CFG               = --cfg=$(GFX_PLATFORM)

# Default target

.PHONY: all
all: lib examples doc

# Dependency handling

.PHONY: submodule
submodule-update:
	@git submodule init
	@git submodule update --recursive

$(DEPS_DIR)/gl-rs/README.md: submodule-update

.PHONY: deps
deps: $(DEPS_DIR)/gl-rs/README.md
	$(MAKE) lib -C $(DEPS_DIR)/gl-rs
	$(MAKE) lib -C $(DEPS_DIR)/glfw-rs

# Library compilation

$(COMM_OUT): $(COMM_INPUT)
	mkdir -p $(LIB_DIR)
	$(RUSTC) --out-dir=$(LIB_DIR) -O $(COMM_FILE)

$(DEVICE_OUT): $(COMM_OUT) $(DEVICE_INPUT)
	mkdir -p $(LIB_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(DEVICE_CFG) -O $(DEVICE_FILE)

$(GLFW_PLATFORM_OUT): $(DEVICE_OUT) $(GLFW_PLATFORM_INPUT)
	mkdir -p $(LIB_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(LIB_CFG) -O $(GLFW_PLATFORM_FILE)

$(RENDER_OUT): $(DEVICE_OUT) $(COMM_OUT) $(RENDER_INPUT)
	mkdir -p $(LIB_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(LIB_CFG) -O $(RENDER_FILE)

$(LIB_OUT): $(DEVICE_OUT) $(GLFW_PLATFORM_OUT) $(RENDER_OUT) $(LIB_INPUT)
	mkdir -p $(LIB_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --out-dir=$(LIB_DIR) $(LIB_CFG) -O $(LIB_FILE)

# .PHONY: lib
lib: $(LIB_OUT)

# Tests

$(COMM_TEST_OUT): $(COMM_INPUT)
	mkdir -p $(TEST_DIR)
	$(RUSTC) --test --out-dir=$(TEST_DIR) -O $(COMM_FILE)
	./$(COMM_TEST_OUT)

$(DEVICE_TEST_OUT): $(COMM_OUT) $(DEVICE_INPUT)
	mkdir -p $(TEST_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --test --out-dir=$(TEST_DIR) $(DEVICE_CFG) -O $(DEVICE_FILE)
	./$(DEVICE_TEST_OUT)

$(GLFW_PLATFORM_TEST_OUT): $(DEVICE_OUT) $(GLFW_PLATFORM_INPUT)
	mkdir -p $(TEST_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --test --out-dir=$(TEST_DIR) $(LIB_CFG) -O $(GLFW_PLATFORM_FILE)
	./$(GLFW_PLATFORM_TEST_OUT)

$(RENDER_TEST_OUT): $(DEVICE_OUT) $(COMM_OUT) $(RENDER_INPUT)
	mkdir -p $(TEST_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --test --out-dir=$(TEST_DIR) $(LIB_CFG) -O $(RENDER_FILE)
	./$(RENDER_TEST_OUT)

$(LIB_TEST_OUT): $(DEVICE_OUT) $(GLFW_PLATFORM_OUT) $(RENDER_OUT) $(LIB_INPUT)
	mkdir -p $(TEST_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --test --out-dir=$(TEST_DIR) $(LIB_CFG) -O $(LIB_FILE)
	./$(LIB_TEST_OUT)

.PHONY: test
test: $(COMM_TEST_OUT) $(DEVICE_TEST_OUT) $(GLFW_PLATFORM_TEST_OUT) $(RENDER_TEST_OUT) $(LIB_TEST_OUT)

# Documentation generation

.PHONY: doc
doc:
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) $(LIB_INCLUDE_FLAGS) $(GFX_CFG) -o $(DOC_DIR) $(LIB_FILE)

# Example compilation

$(EXAMPLE_FILES): lib
	mkdir -p $(EXAMPLES_DIR)
	$(RUSTC) $(EXAMPLE_INCLUDE_FLAGS) --out-dir=$(EXAMPLES_DIR) $@

.PHONY: examples
examples: $(EXAMPLE_FILES)

# Cleanup

.PHONY: clean-deps
clean-deps:
	$(MAKE) clean -C $(DEPS_DIR)/gl-rs
	$(MAKE) clean -C $(DEPS_DIR)/glfw-rs

.PHONY: clean
clean:
	rm -rf $(LIB_DIR)
	rm -rf $(TEST_DIR)
	rm -rf $(EXAMPLES_DIR)
	rm -rf $(DOC_DIR)
