 # GFX Examples

A collection of GFX examples for using the different different GFX API's.

## Getting Started
The gfx-rs git repository contains a number of examples.
Those examples are automatically downloaded if you clone the gfx directory:

	$ cd <my_dir>
	$ git clone https://github.com/gfx-rs/gfx

where `<my_dir>` is a directory name of your choice. Once gfx is downloaded you can build any of the gfx examples.

### Why three different example directories?

The examples are split across three directories, each pertaining to the GFX library they are using.

1. examples/corell contains low level examples.
2. examples/render shows how to use the render crate directly.
3. examples/support shows how to utilize the support module, showing you how you can build an
application using minimal setup.

To the run the examples, set your working directory to the examples/ directory and execute
`cargo run --bin *` where * is the example you wan to run.

For example, try:

	$ cd <my_dir>/examples
	$ cargo run --bin triangle

or

	$ cd <my_dir>/examples
	$ cargo run --bin trianglell

If you compile the example for the first time, it may take some while since all dependencies must be compiled too.
