# Quad

The following image is an output of `cargo run --bin quad --features=gl`:

![screenshot](screenshot.png "Quad")

If the environment variable `DIRECT_DISPLAY` is setted, the example will try to
present the image directly on the terminal without windowing system. This path
will work only with the vulkan backend, so the `--features=vulkan` flag need to
be used. Like the standard winit path, pressing the key "Esc" will terminate the application.
Due to input gathering nature, the application will collect all
the inputs while running, so the user will not be able to do anything in the meantime.
For this reason, the application will exit automatically after 10 seconds to
avoid user get stucked.
