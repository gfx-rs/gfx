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

uint3 GetImageCopyDst(uint3 dispatch_thread_id)
{
    return uint3(ImageCopies.Dst.xy + dispatch_thread_id.xy, ImageCopies.Dst.z);
}

uint3 GetImageCopySrc(uint3 dispatch_thread_id)
{
    return uint3(ImageCopies.Src.xy + dispatch_thread_id.xy, ImageCopies.Src.z);
}

uint3 GetImageDst(uint3 dispatch_thread_id)
{
    return uint3(BufferImageCopies.ImageOffset.xy + dispatch_thread_id.xy, BufferImageCopies.ImageOffset.z);
}

uint3 GetImageSrc(uint3 dispatch_thread_id)
{
    return uint3(BufferImageCopies.ImageOffset.xy + dispatch_thread_id.xy, BufferImageCopies.ImageOffset.z);
}

uint GetBufferDst128(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 16 + dispatch_thread_id.y * 16 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}
uint GetBufferSrc128(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 16 + dispatch_thread_id.y * 16 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}

uint GetBufferDst64(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 8 + dispatch_thread_id.y * 8 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}
uint GetBufferSrc64(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 8 + dispatch_thread_id.y * 8 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}

uint GetBufferDst32(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 4 + dispatch_thread_id.y * 4 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}
uint GetBufferSrc32(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 4 + dispatch_thread_id.y * 4 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}

uint GetBufferDst16(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 4 + dispatch_thread_id.y * 2 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}
uint GetBufferSrc16(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 4 + dispatch_thread_id.y * 2 * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}

uint GetBufferDst8(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 4 + dispatch_thread_id.y * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}
uint GetBufferSrc8(uint3 dispatch_thread_id)
{
    return BufferImageCopies.BufferVars.x + dispatch_thread_id.x * 4 + dispatch_thread_id.y * max(BufferImageCopies.BufferVars.y, BufferImageCopies.ImageExtent.x);
}


uint4 Uint32ToUint8x4(uint data)
{
    return (data >> uint4(0, 8, 16, 24)) & 0xFF;
}

uint2 Uint32ToUint16x2(uint data)
{
    return (data >> uint2(0, 16)) & 0xFFFF;
}

uint Uint8x4ToUint32(uint4 data)
{
    return dot(min(data, 0xFF), 1 << uint4(0, 8, 16, 24));
}

uint Uint16x2ToUint32(uint2 data)
{
    return dot(min(data, 0xFFFF), 1 << uint2(0, 16));
}

uint2 Uint16ToUint8x2(uint data)
{
    return (data >> uint2(0, 8)) & 0xFF;
}

uint Uint8x2ToUint16(uint2 data)
{
    return dot(min(data, 0xFF), 1 << uint2(0, 8));
}

// Buffers are always R32-aligned
ByteAddressBuffer   BufferCopySrc : register(t0);
RWByteAddressBuffer BufferCopyDst : register(u0);

Texture2DArray<uint4>   ImageCopySrc     : register(t0);
RWTexture2DArray<uint>  ImageCopyDstR    : register(u0);
RWTexture2DArray<uint2> ImageCopyDstRg   : register(u0);
RWTexture2DArray<uint4> ImageCopyDstRgba : register(u0);

// Image<->Image copies
[numthreads(1, 1, 1)]
void cs_copy_image2d_r8g8_image2d_r16(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstR[dst_idx] = Uint8x2ToUint16(ImageCopySrc[src_idx]);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16_image2d_r8g8(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstRg[dst_idx] = Uint16ToUint8x2(ImageCopySrc[src_idx]);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8g8b8a8_image2d_r32(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstR[dst_idx] = Uint8x4ToUint32(ImageCopySrc[src_idx]);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8g8b8a8_image2d_r16g16(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstRg[dst_idx] = Uint32ToUint16x2(Uint8x4ToUint32(ImageCopySrc[src_idx]));
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16g16_image2d_r32(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstR[dst_idx] = Uint16x2ToUint32(ImageCopySrc[src_idx]);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16g16_image2d_r8g8b8a8(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstRgba[dst_idx] = Uint32ToUint8x4(Uint16x2ToUint32(ImageCopySrc[src_idx]));
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r32_image2d_r16g16(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstRg[dst_idx] = Uint32ToUint16x2(ImageCopySrc[src_idx]);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r32_image2d_r8g8b8a8(uint3 dispatch_thread_id : SV_DispatchThreadID)
{
    uint3 dst_idx = GetImageCopyDst(dispatch_thread_id);
    uint3 src_idx = GetImageCopySrc(dispatch_thread_id);

    ImageCopyDstRgba[dst_idx] = Uint32ToUint8x4(ImageCopySrc[src_idx]);
}

// Buffer<->Image copies

// R32G32B32A32
// TODO: correct, but slow
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r32g32b32a32(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc128(dispatch_thread_id);

    ImageCopyDstRgba[dst_idx] = uint4(
        BufferCopySrc.Load(src_idx),
        BufferCopySrc.Load(src_idx + 1 * 4),
        BufferCopySrc.Load(src_idx + 2 * 4),
        BufferCopySrc.Load(src_idx + 3 * 4)
    );
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r32g32b32a32_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst128(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(dispatch_thread_id);

    uint4 data = ImageCopySrc[src_idx];

    BufferCopyDst.Store(dst_idx,         data.x);
    BufferCopyDst.Store(dst_idx + 1 * 4, data.y);
    BufferCopyDst.Store(dst_idx + 2 * 4, data.z);
    BufferCopyDst.Store(dst_idx + 3 * 4, data.w);
}

// R32G32
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r32g32(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc64(dispatch_thread_id);

    ImageCopyDstRg[dst_idx] = uint2(
        BufferCopySrc.Load(src_idx),
        BufferCopySrc.Load(src_idx + 1 * 4)
    );
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r32g32_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst64(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(dispatch_thread_id);

    uint2 data = ImageCopySrc[src_idx].rg;

    BufferCopyDst.Store(dst_idx        , data.x);
    BufferCopyDst.Store(dst_idx + 1 * 4, data.y);
}

// R16G16B16A16
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r16g16b16a16(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc64(dispatch_thread_id);

    ImageCopyDstRgba[dst_idx] = uint4(
        Uint32ToUint16x2(BufferCopySrc.Load(src_idx)),
        Uint32ToUint16x2(BufferCopySrc.Load(src_idx + 1 * 4))
    );
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16g16b16a16_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst64(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(dispatch_thread_id);

    uint4 data = ImageCopySrc[src_idx];

    BufferCopyDst.Store(dst_idx,         Uint16x2ToUint32(data.xy));
    BufferCopyDst.Store(dst_idx + 1 * 4, Uint16x2ToUint32(data.zw));
}

// R32
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r32(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc32(dispatch_thread_id);

    ImageCopyDstR[dst_idx] = BufferCopySrc.Load(src_idx);
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r32_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst32(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(dispatch_thread_id);

    BufferCopyDst.Store(dst_idx, ImageCopySrc[src_idx].r);
}

// R16G16
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r16g16(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc32(dispatch_thread_id);

    ImageCopyDstRg[dst_idx] = Uint32ToUint16x2(BufferCopySrc.Load(src_idx));
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16g16_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst32(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(dispatch_thread_id);

    BufferCopyDst.Store(dst_idx, Uint16x2ToUint32(ImageCopySrc[src_idx].xy));
}

// R8G8B8A8
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r8g8b8a8(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(dispatch_thread_id);
    uint src_idx = GetBufferSrc32(dispatch_thread_id);

    ImageCopyDstRgba[dst_idx] = Uint32ToUint8x4(BufferCopySrc.Load(src_idx));
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8g8b8a8_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst32(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(dispatch_thread_id);

    BufferCopyDst.Store(dst_idx, Uint8x4ToUint32(ImageCopySrc[src_idx]));
}

// R16
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r16(uint3 dispatch_thread_id : SV_DispatchThreadID) {
//    uint src_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;
    uint3 dst_idx = GetImageDst(uint3(2, 1, 0) * dispatch_thread_id);
    uint src_idx = GetBufferSrc16(dispatch_thread_id);
    uint2 data = Uint32ToUint16x2(BufferCopySrc.Load(src_idx));

    ImageCopyDstR[dst_idx                 ] = data.x;
    ImageCopyDstR[dst_idx + uint3(1, 0, 0)] = data.y;
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r16_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    //uint dst_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;
    uint dst_idx = GetBufferDst16(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(uint3(2, 1, 0) * dispatch_thread_id);

    uint upper = ImageCopySrc[src_idx].r;
    uint lower = ImageCopySrc[src_idx + uint3(1, 0, 0)].r;

    BufferCopyDst.Store(dst_idx, Uint16x2ToUint32(uint2(upper, lower)));
}

// R8G8
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r8g8(uint3 dispatch_thread_id : SV_DispatchThreadID) {
//    uint src_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;
    uint3 dst_idx = GetImageDst(uint3(2, 1, 0) * dispatch_thread_id);
    uint src_idx = GetBufferSrc16(dispatch_thread_id);

    uint4 data = Uint32ToUint8x4(BufferCopySrc.Load(src_idx));

    ImageCopyDstRg[dst_idx                 ] = data.xy;
    ImageCopyDstRg[dst_idx + uint3(1, 0, 0)] = data.zw;
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8g8_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
//    uint dst_idx = BufferImageCopies.BufferVars.x + dispatch_thread_id.x + dispatch_thread_id.y * BufferImageCopies.BufferVars.y / 2;
    uint dst_idx = GetBufferDst16(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(uint3(2, 1, 0) * dispatch_thread_id);

    uint2 lower = ImageCopySrc[src_idx].xy;
    uint2 upper = ImageCopySrc[src_idx + uint3(1, 0, 0)].xy;

    BufferCopyDst.Store(dst_idx, Uint8x4ToUint32(uint4(lower.x, lower.y, upper.x, upper.y)));
}

// R8
[numthreads(1, 1, 1)]
void cs_copy_buffer_image2d_r8(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint3 dst_idx = GetImageDst(uint3(4, 1, 0) * dispatch_thread_id);
    uint src_idx = GetBufferSrc8(dispatch_thread_id);
    uint4 data = Uint32ToUint8x4(BufferCopySrc.Load(src_idx));

    ImageCopyDstR[dst_idx              ] = data.x;
    ImageCopyDstR[dst_idx + uint3(1, 0, 0)] = data.y;
    ImageCopyDstR[dst_idx + uint3(2, 0, 0)] = data.z;
    ImageCopyDstR[dst_idx + uint3(3, 0, 0)] = data.w;
}

[numthreads(1, 1, 1)]
void cs_copy_image2d_r8_buffer(uint3 dispatch_thread_id : SV_DispatchThreadID) {
    uint dst_idx = GetBufferDst8(dispatch_thread_id);
    uint3 src_idx = GetImageSrc(uint3(4, 1, 0) * dispatch_thread_id);

    BufferCopyDst.Store(dst_idx, Uint8x4ToUint32(uint4(
        ImageCopySrc[src_idx].r,
        ImageCopySrc[src_idx + uint3(1, 0, 0)].r,
        ImageCopySrc[src_idx + uint3(2, 0, 0)].r,
        ImageCopySrc[src_idx + uint3(3, 0, 0)].r
    )));
}
