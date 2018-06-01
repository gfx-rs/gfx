#include <metal_stdlib>
using namespace metal;

typedef struct {
    uint value;
    uint length;
} FillBufferValue;

kernel void cs_fill_buffer(
    device uint *buffer [[ buffer(0) ]],
    constant FillBufferValue &fill [[ buffer(1) ]],
    uint index [[ thread_position_in_grid ]]
) {
    if (index < fill.length) {
        buffer[index] = fill.value;
    }
}

kernel void cs_copy_buffer(
    device uchar *dest [[ buffer(0) ]],
    device uchar *source [[ buffer(1) ]],
    constant uint &size [[ buffer(2) ]],
    uint index [[ thread_position_in_grid ]]
) {
    if (index < size) {
        dest[index] = source[index];
    }
}
