# Getting Started

## macOS Dependencies

Install the newest XCode from the App Store. This installs the required `metal` developer tools.   

To run the examples, ensure [CMake is installed](https://cmake.org/install/) as it is required for `glsl-to-spirv`.   

## Vulkan Dependencies

First, install the x11 and Vulkan dev libraries.

```bash
# Fedora
sudo dnf install -y libX11-devel vulkan
# Ubuntu
sudo apt install -y libx11-dev libvulkan-dev libxcb1-dev xorg-dev
```

For Linux, a Vulkan compatible driver must also be installed. For example, the open source `mesa-vulkan-drivers` for Intel or Radeon gpu's. The proprietary Nvidia drivers support Vulkan out of the box but, as of time of writing, Nouveau users are currenty limited to OpenGL.

## Usage

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
