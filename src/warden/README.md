# Warden

Warden is the data-driven reference test framework for gfx-rs Hardware Abstraction Layer (`gfx-hal`), heavily inspired by the Wrench component of [WebRender](https://github.com/servo/webrender/). Warden's main purpose is to run a suite of GPU workloads on all native backends supported by the host platform, then match the results against provided expectations. Both the workloads and expectations are backend-agnostic. The backend discovery and initialization is done by the `reftest` binary. All that needs to be done by a developer is typing `make reftests` from the project root and ensuring that every test passes.

Warden has two types of definitions: scene and suite. Both are written in [Ron](https://github.com/ron-rs/ron) format, but technically the code should work with any `serde`-enabled format given minimal tweaking.

## Scene definition

A scene consists of a number of resources and jobs that can be run on them. Resources are buffers, images, render passes, and so on. Jobs are sets of either transfer, compute, or graphics operations. The latter is contained within a single render pass. Please refer to [raw.rs](src/raw.rs) for the formal definition of the scene format. Actual reference scenes can be found in [reftests](../../reftests/scenes).

### Resource states

Internally, a scene has a command buffer to fill up all the initial data for resources. This command buffer needs to change the resource access and image layouts, so we establish a convention here by which every resource has an associated "stable" state that the user (and the reftest framework) promises to deliver at the end of each job.

For images with no source data, the stable layout is `ColorAttachmentOptimal` or `DepthStencilAttachmentOptimal` depending on the format. For sourced images, it's `ShaderReadOnlyOptimal`.

## Test suite

A test suite is just a set of scenes, each with multiple tests. A test is defined as a sequence of jobs being run on the scene and an expectation result. The central suite file can be found in [reftests](../../reftests/suite.ron), and the serialization structures are in [reftest.rs](src/bin/reftest.rs).

## Warning

This gfx-rs component is heavy WIP, provided under no warranty! There is a lot of logic missing, especially with regards to error reporting.
