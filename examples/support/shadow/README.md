# Shadow Example

This example shows multi-threaded rendering for computing the shadows of
multiple lights. A command buffer per each shadow map is composed by a
separate thread and then sent to the device for execution.

Moving the mouse cursor rotates the cubes for your entertainment.

You can switch to single-threaded mode by appending "single" to the command
line. Currently, the overhead of creating the threads seems to be higher
than the benefit from multi-threading. There needs to be a large number of
generated objects in order to get the fork-join model to show a difference.

## Screenshot

![Shadow Example](screenshot.png)
