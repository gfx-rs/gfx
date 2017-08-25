# Particle Example

A simple example showing how use a geometry shader to expand a point list into
quads in order to render a large number of particles efficiently.

Without the geometry shader, all four vertices of each particle's quad would need
to be sent to the GPU every frame. By sending only a single vertex per particle,
the bandwidth requirements are cut down first by a factor of four, as each quad
would require four vertices, and then reduced further by avoiding the need to
send UV data, since it can be generated on the fly in the geometry shader.

## Screenshot

![Particle Example](screenshot.png)
