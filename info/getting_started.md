# Getting Started

## macOS Dependencies

1. Install the newest XCode from the App Store. This installs the required `metal` developer tools.
2. Install the command line tools with `xcode-select --install`. This might do nothing on your machine.
3. If `xcode-select --print-path` prints `/Library/Developer/CommandLineTools…`
4. then run `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`.

To run the examples, ensure [CMake is installed](https://cmake.org/install/) as it is required for `glsl-to-spirv`.

## Vulkan Dependencies

First, install the x11 and Vulkan dev libraries.

```bash
# Fedora
sudo dnf install -y libX11-devel vulkan
# Ubuntu
sudo apt install -y libx11-dev libvulkan-dev libxcb1-dev xorg-dev
# Archlinux
sudo pacman -S vulkan-icd-loader lib32-vulkan-icd-loader vulkan-headers
```

For Linux, a Vulkan compatible driver must also be installed. For example, the open source `mesa-vulkan-drivers` for Intel or Radeon gpu's. The proprietary Nvidia drivers support Vulkan out of the box but, as of time of writing, Nouveau users are currenty limited to OpenGL.

## Usage

As mentioned befored, gfx is a low-level library, not necessarily intended for beginners.
You might want to get a grasp on the fundamental graphics concepts by using [wgpu-rs](https://github.com/gfx-rs/wgpu/tree/master/wgpu).

A good tutorial for learning how to use gfx is [mistodon/gfx-hal-tutorials](https://github.com/mistodon/gfx-hal-tutorials).

The gfx repository contains a number of examples. Those examples are automatically downloaded when the repository is cloned.

To run an example, simply use `cargo run` and specify the backend with `--features {backend}` (where `{backend}` is one of `vulkan`, `dx12`, `metal`, or `gl`). For example:

```bash
git clone https://github.com/gfx-rs/gfx
cd gfx/examples
# macOS
cargo run --bin quad --features metal
# vulkan
cargo run --bin quad --features vulkan
# Windows
cargo run --bin compute --features dx12 1 2 3 4
```

This would run the `quad` example using the Vulkan backend, and then the `compute` example using the Direct3D 12 backend.
