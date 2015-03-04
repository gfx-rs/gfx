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

DIAGRAMS_IN_DOT = $(wildcard diagrams/dot/*.dot)
DIAGRAMS_OUT_PNG = $(patsubst diagrams/dot/%.dot,diagrams/png/%.png,$(DIAGRAMS_IN_DOT))

.PHONY: all
all:
	cargo build

.PHONY: update
update:
	cargo update
	rm -rf doc

.PHONY: test
test:
	cargo test

.PHONY: doc
doc:
	cargo doc

$(DIAGRAMS_OUT_PNG): $(DIAGRAMS_IN_DOT)
	@mkdir -p diagrams/png
	dot -Tpng -o $@ -Gsize=6,6 -Gdpi=100 $<

.PHONY: diagrams
diagrams: $(DIAGRAMS_OUT_PNG)

.PHONY: clean-diagrams
clean-diagrams:
	rm -rf diagrams/png

.PHONY: clean
clean: clean-diagrams
	cargo clean
	rm -rf doc

.PHONY: travis
travis: test doc
