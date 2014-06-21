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

.PHONY= all help lib deps examples

all: deps lib examples

lib:
	mkdir -p bin
	rustc -L bin -L deps --out-dir=bin --cfg=glfw --cfg=gl src/gfx/lib.rs

dep-gl:
	(cd deps/gl-rs && make submodule-update lib && cp lib/*.rlib ..)

dep-glfw:
	(cd deps/glfw-rs && make lib && cp lib/*.rlib ..)

clean:
	rm -f deps/*.rlib

deps: clean dep-gl dep-glfw

examples:
	rustc -L bin -L deps -o bin/ex-triangle src/examples/triangle/main.rs

help:
	echo "Valid commands are all, lib, dep-*, deps, clean, examples"
