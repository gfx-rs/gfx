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
GL_EXTENSIONS = GL_EXT_texture_filter_anisotropic

SRC_DIR               = src
DEPS_DIR              = deps
COMM_FILE             = $(SRC_DIR)/comm/lib.rs
DEVICE_FILE           = $(SRC_DIR)/device/lib.rs
GLFW_PLATFORM_FILE    = $(SRC_DIR)/glfw_platform/lib.rs
RENDER_FILE           = $(SRC_DIR)/render/lib.rs
LIB_FILE              = $(SRC_DIR)/gfx/lib.rs
MACRO_FILE            = $(SRC_DIR)/gfx_macros/lib.rs
MACRO_TEST_FILE       = $(SRC_DIR)/gfx_macros_test/lib.rs

COMM_INPUT            = $(SRC_DIR)/comm/*.rs
DEVICE_INPUT          = $(SRC_DIR)/device/*.rs $(SRC_DIR)/device/gl/*.rs
GLFW_PLATFORM_INPUT   = $(SRC_DIR)/glfw_platform/*.rs
RENDER_INPUT          = $(SRC_DIR)/render/*.rs
LIB_INPUT             = $(SRC_DIR)/gfx/*.rs
MACRO_INPUT           = $(SRC_DIR)/gfx_macros/*.rs
MACRO_TEST_INPUT      = $(SRC_DIR)/gfx_macros_test/*.rs

DEPS_LIB_SEARCH_PATHS = $(DEPS_DIR)/gl-rs/lib $(DEPS_DIR)/glfw-rs/lib
DEPS_LIB_SEARCH_FLAGS = $(patsubst %,-L %, $(DEPS_LIB_SEARCH_PATHS))

LIB_DIR               = lib
COMM_OUT              = $(LIB_DIR)/$(shell $(RUSTC) --print-file-name $(COMM_FILE))
DEVICE_OUT            = $(LIB_DIR)/$(shell $(RUSTC) --print-file-name $(DEVICE_FILE))
GLFW_PLATFORM_OUT     = $(LIB_DIR)/$(shell $(RUSTC) --print-file-name $(GLFW_PLATFORM_FILE))
RENDER_OUT            = $(LIB_DIR)/$(shell $(RUSTC) --print-file-name $(RENDER_FILE))
LIB_OUT               = $(LIB_DIR)/$(shell $(RUSTC) --print-file-name $(LIB_FILE))
MACRO_OUT             = $(LIB_DIR)/$(shell $(RUSTC) --print-file-name $(MACRO_FILE))

TEST_DIR              = test
COMM_TEST_OUT         = $(TEST_DIR)/$(shell $(RUSTC) --print-file-name --test $(COMM_FILE))
DEVICE_TEST_OUT       = $(TEST_DIR)/$(shell $(RUSTC) --print-file-name --test $(DEVICE_FILE))
GLFW_PLATFORM_TEST_OUT= $(TEST_DIR)/$(shell $(RUSTC) --print-file-name --test $(GLFW_PLATFORM_FILE))
RENDER_TEST_OUT       = $(TEST_DIR)/$(shell $(RUSTC) --print-file-name --test $(RENDER_FILE))
LIB_TEST_OUT          = $(TEST_DIR)/$(shell $(RUSTC) --print-file-name --test $(LIB_FILE))
MACRO_TEST_OUT        = $(TEST_DIR)/$(shell $(RUSTC) --print-file-name --test $(MACRO_TEST_FILE))

DOC_DIR               = doc
COMM_DOC_OUT          = $(DOC_DIR)/$(shell $(RUSTC) --print-crate-name $(COMM_FILE))
DEVICE_DOC_OUT        = $(DOC_DIR)/$(shell $(RUSTC) --print-crate-name $(DEVICE_FILE))
GLFW_PLATFORM_DOC_OUT = $(DOC_DIR)/$(shell $(RUSTC) --print-crate-name $(GLFW_PLATFORM_FILE))
RENDER_DOC_OUT        = $(DOC_DIR)/$(shell $(RUSTC) --print-crate-name $(RENDER_FILE))
LIB_DOC_OUT           = $(DOC_DIR)/$(shell $(RUSTC) --print-crate-name $(LIB_FILE))
MACRO_DOC_OUT         = $(DOC_DIR)/$(shell $(RUSTC) --print-crate-name $(MACRO_FILE))

LIB_INCLUDE_FLAGS     = -L $(LIB_DIR) $(DEPS_LIB_SEARCH_FLAGS)

GFX_API               ?= gl
GFX_PLATFORM          ?= glfw

DEVICE_CFG            = --cfg=$(GFX_API)
LIB_CFG               = --cfg=$(GFX_PLATFORM)

# Default target

.PHONY: all
all: lib doc

# Friendly initialization

.PHONY: init
init: deps all

# Dependency handling

.PHONY: submodule-update
submodule-update:
	@git submodule init
	@git submodule update --recursive

$(DEPS_DIR)/gl-rs/README.md: submodule-update

.PHONY: deps
deps: $(DEPS_DIR)/gl-rs/README.md
	$(MAKE) lib -C $(DEPS_DIR)/gl-rs GL_EXTENSIONS=$(GL_EXTENSIONS)
	$(MAKE) lib -C $(DEPS_DIR)/glfw-rs

.PHONY: clean-deps
clean-deps:
	$(MAKE) clean -C $(DEPS_DIR)/gl-rs
	$(MAKE) clean -C $(DEPS_DIR)/glfw-rs

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

$(MACRO_OUT): $(MACRO_INPUT)
	mkdir -p $(LIB_DIR)
	$(RUSTC) --out-dir=$(LIB_DIR) -O $(MACRO_FILE)

.PHONY: lib
lib: $(LIB_OUT)

.PHONY: clean-lib
clean-lib:
	rm -rf $(LIB_DIR)

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

$(MACRO_TEST_OUT): $(LIB_OUT) $(MACRO_OUT) $(MACRO_TEST_INPUT)
	mkdir -p $(TEST_DIR)
	$(RUSTC) $(LIB_INCLUDE_FLAGS) --test --out-dir=$(TEST_DIR) $(LIB_CFG) -O $(MACRO_TEST_FILE)
	./$(MACRO_TEST_OUT)

.PHONY: test
test: $(COMM_TEST_OUT) $(DEVICE_TEST_OUT) $(GLFW_PLATFORM_TEST_OUT) $(RENDER_TEST_OUT) $(LIB_TEST_OUT) $(MACRO_TEST_OUT)

.PHONY: clean-test
clean-test:
	rm -rf $(TEST_DIR)

# Documentation generation

$(COMM_DOC_OUT): $(COMM_INPUT)
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) -o $(DOC_DIR) $(COMM_FILE)

$(DEVICE_DOC_OUT): $(COMM_OUT) $(DEVICE_INPUT)
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) $(LIB_INCLUDE_FLAGS) $(DEVICE_CFG) -o $(DOC_DIR) $(DEVICE_FILE)

$(GLFW_PLATFORM_DOC_OUT): $(DEVICE_OUT) $(GLFW_PLATFORM_INPUT)
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) $(LIB_INCLUDE_FLAGS) $(LIB_CFG) -o $(DOC_DIR) $(RENDER_FILE)

$(RENDER_DOC_OUT): $(DEVICE_OUT) $(COMM_OUT) $(RENDER_INPUT)
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) $(LIB_INCLUDE_FLAGS) $(LIB_CFG) -o $(DOC_DIR) $(GLFW_PLATFORM_FILE)

$(LIB_DOC_OUT): $(DEVICE_OUT) $(GLFW_PLATFORM_OUT) $(RENDER_OUT) $(LIB_INPUT)
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) $(LIB_INCLUDE_FLAGS) $(LIB_CFG) -o $(DOC_DIR) $(LIB_FILE)

$(MACRO_DOC_OUT): $(MACRO_INPUT)
	mkdir -p $(DOC_DIR)
	$(RUSTDOC) -o $(DOC_DIR) $(MACRO_FILE)

.PHONY: doc
doc: $(COMM_DOC_OUT) $(DEVICE_DOC_OUT) $(GLFW_PLATFORM_DOC_OUT) $(RENDER_DOC_OUT) $(LIB_DOC_OUT) $(MACRO_DOC_OUT)

.PHONY: clean-doc
clean-doc:
	rm -rf $(DOC_DIR)

# Cleanup

.PHONY: clean
clean: clean-lib clean-test clean-doc
