<!--
    Copyright 2014 The Gfx-rs Developers.

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
-->

## Contributing to gfx-rs

### Communication

General-purpose chat is on [Gitter](https://gitter.im/gfx-rs/gfx-rs). It has all the project activity displayed on the right side for your convenience. Note that it embeds images and gists automatically, so post bare links with discretion.

If you've got the code, making a pull request to discuss things on the way may be more efficient than posting it to the chat.

There is also a [Waffle board](https://waffle.io/gfx-rs/gfx-rs), which you can use conveniently to track the whole picture.

Finally, feel free to hop on [#rust-gamedev](http://chat.mibbit.com/?server=irc.mozilla.org&channel=%23rust-gamedev).

### Directory Structure 

* _examples_ : GFX's examples
* _tests_ : GFX's tests
* _info_ : Information and documentation
* _src_ : gfx_app, an application framework for GFX
* _src/core_ : gfx_core, core structures and the interface that backends must provide
* _src/backend_ : The backends implementations
* _src/render_ : The main gfx package
* _src/window_ : Different backends to create windows and initialize their graphics

### Code

gfx-rs adheres to [Rust Coding Guidelines](http://aturon.github.io/).