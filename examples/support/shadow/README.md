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
