cbuffer BufferImageCopy : register(b0) {
    uint2 BufferSize;
    uint2 ImageOffset;
};

StructuredBuffer<uint> CopySrc : register(t0);
RWTexture2D<uint> CopyDst : register(u0);

// TODO: correct, but slow
[numthreads(1, 1, 1)]
void cs_copy_buffer_image_2d(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint2 idx = ImageOffset + dispatch_thread_id.xy;

    CopyDst[idx] = CopySrc[BufferSize.x + idx.x + idx.y * BufferSize.y];
}
