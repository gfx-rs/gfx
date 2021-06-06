# Quad

The following image is an output of `cargo run --bin quad --features=gl`:

![screenshot](screenshot.png "Quad")

If the environment variable `DIRECT_DISPLAY` is setted, the example will try to
present the image directly on the terminal without windowing system. This path
will work only with the vulkan backend, so the `--features=vulkan` flag need to
be used. When launched in this mode, the Gfx logo will be showed for 5 seconds and then the example will close.
