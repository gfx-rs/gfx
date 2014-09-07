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

all: build

build:
	(cd triangle && cargo build)
	(cd cube && cargo build)
	(cd gl-init && cargo build)
	(cd terrain && cargo build)
	(cd performance && cargo build)

update:
	(cd triangle && cargo update)
	(cd cube && cargo update)
	(cd gl-init && cargo update)
	(cd terrain && cargo update)
	(cd performance && cargo update)

clean:
	(cd triangle && cargo clean)
	(cd cube && cargo clean)
	(cd gl-init && cargo clean)
	(cd terrain && cargo clean)
	(cd performance && cargo clean)
