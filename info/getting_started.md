# Getting Started

## Vulkan Dependencies

First, install the x11 and Vulkan dev libraries.

For Fedora

```bash
sudo dnf install -y libX11-devel vulkan
```

For Ubuntu

```bash
sudo apt install -y libx11-dev libvulkan-dev
```

For Linux, a Vulkan compatible driver must also be installed. For example, the open source ``` mesa-vulkan-drivers ``` for Intel or Radeon gpu's. The proprietary Nvidia drivers support Vulkan out of the box but, as of time of writing, Nouveau users are currenty limited to OpenGL.

## Usage

The gfx repository contains a number of examples. Those examples are automatically downloaded when the repository is cloned.

To run an example, simply use `cargo run` and specify the backend with `--features {backend}` (where `{backend}` is one of `vulkan`, `dx12`, `metal`, or `gl`). For example:

```bash
git clone https://github.com/gfx-rs/gfx
cd gfx/examples/hal
cargo run --bin quad --features vulkan
cargo run --bin compute --features dx12 1 2 3 4
```

This would run the `quad` example using the Vulkan backend, and then the `compute` example using the Direct3D 12 backend.
