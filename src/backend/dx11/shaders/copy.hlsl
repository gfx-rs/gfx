struct BufferCopy {
    uint4 SrcDst;
};

struct ImageCopy {
    uint4 Src;
    uint4 Dst;
};

struct BufferImageCopy {
    // x=offset, yz=size
    uint4 BufferVars;
    uint4 ImageOffset;
    uint4 ImageExtent;
};

cbuffer CopyConstants : register(b0) {
    BufferCopy BufferCopies;
    ImageCopy ImageCopies;
    BufferImageCopy BufferImageCopies;
};

uint2 GetImageDst(uint3 dispatch_thread_id)
{
    return BufferImageCopies.ImageOffset.xy + dispatch_thread_id.xy;
}

uint2 GetImageSrc(uint3 dispatch_thread_id)
{
    return BufferImageCopies.ImageOffset.xy + dispatch_thread_id.xy;
}

uint GetBufferDst(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y;
}

uint GetBufferSrc(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y;
}

uint Uint4ToUint(uint4 data)
{
    data.x = min(data.x, 0x000000ff);
    data.y = min(data.y, 0x000000ff);
    data.z = min(data.z, 0x000000ff);
    data.w = min(data.w, 0x000000ff);

    uint output = (data.x        |
                  (data.y << 8)  |
                  (data.z << 16) |
                  (data.w << 24));

    return output;
}

uint4 UintToUint4(uint data)
{
    return uint4((data & 0xff000000) >> 24, (data & 0xff0000) >> 16, (data & 0xff00) >> 8, data & 0xff);
}

uint2 UintToUint2(uint data)
{
    return uint2((data >> 16) & 0x0000ffff, data & 0x0000ffff);
}

uint Uint2ToUint(uint2 data)
{
    data.x = min(data.x, 0x0000ffff);
    data.y = min(data.y, 0x0000ffff);

    uint output = (data.x         |
                  (data.y << 16));

    return output;
}

// Buffers are always R32-aligned
StructuredBuffer<uint> BufferCopySrc : register(t0);
RWBuffer<uint>         BufferCopyDst: register(u0);

// R32
Texture2D<uint>    ImageCopySrcR32 : register(t0);
RWTexture2D<uint>  ImageCopyDstR32 : register(u0);

// TODO: correct, but slow
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r32(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint2 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc(dispatch_thread_id);

    ImageCopyDstR32[dst_idx] = BufferCopySrc[src_idx];
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r32_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst(dispatch_thread_id);
    uint2 src_idx = GetImageSrc(dispatch_thread_id);

    BufferCopyDst[dst_idx] = ImageCopySrcR32[src_idx];
}

// R16G16
Texture2D<uint2>    ImageCopySrcR16G16 : register(t0);
RWTexture2D<uint2>  ImageCopyDstR16G16 : register(u0);

// TODO: correct, but slow
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r16g16(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint2 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc(dispatch_thread_id);

    ImageCopyDstR16G16[dst_idx] = UintToUint2(BufferCopySrc[src_idx]);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16g16_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst(dispatch_thread_id);
    uint2 src_idx = GetImageSrc(dispatch_thread_id);

    BufferCopyDst[dst_idx] = Uint2ToUint(ImageCopySrcR16G16[src_idx].yx);
}

// R16
Texture2D<uint>   ImageCopySrcR16 : register(t0);
RWTexture2D<uint> ImageCopyDstR16 : register(u0);

[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r16(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint src_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;

    uint2 data = UintToUint2(BufferCopySrc[src_idx]);

    ImageCopyDstR16[GetImageDst(uint3(2, 1, 0) * dispatch_thread_id + uint3(0, 0, 0))] = data.y;
    ImageCopyDstR16[GetImageDst(uint3(2, 1, 0) * dispatch_thread_id + uint3(1, 0, 0))] = data.x;
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;

    uint upper = ImageCopySrcR16[GetImageSrc(uint3(2, 1, 0) * dispatch_thread_id + uint3(0, 0, 0))];
    uint lower = ImageCopySrcR16[GetImageSrc(uint3(2, 1, 0) * dispatch_thread_id + uint3(1, 0, 0))];
    uint data = Uint2ToUint(uint2(upper, lower));

    BufferCopyDst[dst_idx] = data;
}

// R8G8
Texture2D<uint2>   ImageCopySrcR8G8 : register(t0);
RWTexture2D<uint2> ImageCopyDstR8G8 : register(u0);

[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r8g8(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint src_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;

    uint4 data = UintToUint4(BufferCopySrc[src_idx]);

    ImageCopyDstR8G8[GetImageDst(uint3(2, 1, 0) * dispatch_thread_id + uint3(0, 0, 0))] = data.xy;
    ImageCopyDstR8G8[GetImageDst(uint3(2, 1, 0) * dispatch_thread_id + uint3(1, 0, 0))] = data.zw;
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8g8_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;

    uint2 lower = ImageCopySrcR8G8[GetImageSrc(uint3(2, 1, 0) * dispatch_thread_id + uint3(0, 0, 0))].yx;
    uint2 upper = ImageCopySrcR8G8[GetImageSrc(uint3(2, 1, 0) * dispatch_thread_id + uint3(1, 0, 0))].yx;
    uint data = Uint4ToUint(uint4(upper.x, upper.y, lower.x, lower.y));

    BufferCopyDst[dst_idx] = data;
}

// R8
Texture2D<uint>   ImageCopySrcR8 : register(t0);
RWTexture2D<uint> ImageCopyDstR8 : register(u0);

[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r8(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint src_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 4;
    uint4 data = UintToUint4(BufferCopySrc[src_idx]);

    ImageCopyDstR8[GetImageDst(uint3(4, 1, 0) * dispatch_thread_id + uint3(0, 0, 0))] = data.w;
    ImageCopyDstR8[GetImageDst(uint3(4, 1, 0) * dispatch_thread_id + uint3(1, 0, 0))] = data.z;
    ImageCopyDstR8[GetImageDst(uint3(4, 1, 0) * dispatch_thread_id + uint3(2, 0, 0))] = data.y;
    ImageCopyDstR8[GetImageDst(uint3(4, 1, 0) * dispatch_thread_id + uint3(3, 0, 0))] = data.x;
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 4;

    uint src_1 = ImageCopySrcR8[GetImageSrc(uint3(4, 1, 0) * dispatch_thread_id + uint3(0, 0, 0))];
    uint src_2 = ImageCopySrcR8[GetImageSrc(uint3(4, 1, 0) * dispatch_thread_id + uint3(1, 0, 0))];
    uint src_3 = ImageCopySrcR8[GetImageSrc(uint3(4, 1, 0) * dispatch_thread_id + uint3(2, 0, 0))];
    uint src_4 = ImageCopySrcR8[GetImageSrc(uint3(4, 1, 0) * dispatch_thread_id + uint3(3, 0, 0))];

    BufferCopyDst[dst_idx] = Uint4ToUint(uint4(src_1, src_2, src_3, src_4));
}
