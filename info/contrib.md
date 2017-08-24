## Contributing to gfx-rs

### Communication

General-purpose chat is on [Gitter](https://gitter.im/gfx-rs/gfx-rs). It has all the project activity displayed on the right side for your convenience. Note that it embeds images and gists automatically, so post bare links with discretion.

If you've got the code, making a pull request to discuss things on the way may be more efficient than posting it to the chat.

There is also a [Waffle board](https://waffle.io/gfx-rs/gfx-rs), which you can use conveniently to track the whole picture.

Finally, feel free to hop on [#rust-gamedev](http://chat.mibbit.com/?server=irc.mozilla.org&channel=%23rust-gamedev).

### Directory Structure 

* _src/backend_ : The different backends GFX supports.
* _src/core_ : core structures and the interface that backends must provide
* _src/corell_ : contains low-level graphics implementation (no resource management overhead).
* _src/macros_ : Macros used internally by GFX.
* _src/support_ : Support code that is used by GFX examples. This package contains all of the
  non-essential functionality for running the GFX examples. Feel free to use this code in your own
  project.
* _info_ : Information and documentation.
* _src/render_ : The main gfx package. This is where all user code that uses gfx::* lives.
* _src/window_ : Different backends to create windows and initialize their graphics

### Code

gfx-rs adheres to [Rust Coding Guidelines](http://aturon.github.io/).
