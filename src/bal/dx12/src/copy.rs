use bal::{buffer, image};
use std::cmp;
use std::ops::Range;
use winapi::um::d3d12;

#[derive(Clone, Copy)]
pub struct Image {
    pub levels: image::Level,
    pub layers: image::Layer,
    pub bytes_per_block: u32,
    // Dimension of a texel block (compressed formats).
    pub block_dim: (u32, u32),
}

impl Image {
    pub fn calc_subresource(&self, mip_level: u32, layer: u32, plane: u32) -> u32 {
        mip_level + (layer * self.levels) + (plane * self.levels * self.layers)
    }
}

#[derive(Clone)]
pub struct CopyRegion {
    pub footprint_offset: u64,
    pub footprint: image::Extent,
    pub row_pitch: u32,
    pub img_subresource: u32,
    pub img_offset: image::Offset,
    pub buf_offset: image::Offset,
    pub copy_extent: image::Extent,
}

/// Bundles together all the parameters needed to copy a buffer
/// to an image or vice-versa.
#[derive(Clone, Debug)]
pub struct BufferImageCopy {
    /// Buffer ofset in bytes.
    pub buffer_offset: buffer::Offset,
    /// Width of a buffer 'row' in texels.
    pub buffer_width: u32,
    /// Height of a buffer 'image slice' in texels.
    pub buffer_height: u32,
    /// The offset of the portion of the image to copy.
    pub image_offset: image::Offset,
    /// Size of the portion of the image to copy.
    pub image_extent: image::Extent,
    /// Selected mipmap level
    pub image_level: image::Level,
    /// Included array levels
    pub image_layers: Range<image::Layer>,
}

fn div(a: u32, b: u32) -> u32 {
    (a + b - 1) / b
}

fn up_align(x: u32, alignment: u32) -> u32 {
    (x + alignment - 1) & !(alignment - 1)
}

pub fn split_buffer_copy(copies: &mut Vec<CopyRegion>, r: &BufferImageCopy, image: Image) {
    let buffer_width = if r.buffer_width == 0 {
        r.image_extent.width
    } else {
        r.buffer_width
    };
    let buffer_height = if r.buffer_height == 0 {
        r.image_extent.height
    } else {
        r.buffer_height
    };
    let image_extent_aligned = image::Extent {
        width: up_align(r.image_extent.width, image.block_dim.0 as _),
        height: up_align(r.image_extent.height, image.block_dim.1 as _),
        depth: r.image_extent.depth,
    };
    let row_pitch = div(buffer_width, image.block_dim.0 as _) * image.bytes_per_block;
    let slice_pitch = div(buffer_height, image.block_dim.1 as _) * row_pitch;
    let is_pitch_aligned = row_pitch % d3d12::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT == 0;

    for layer in r.image_layers.clone() {
        let img_subresource = image.calc_subresource(r.image_level as _, layer as _, 0);
        let layer_relative = layer - r.image_layers.start;
        let layer_offset =
            r.buffer_offset as u64 + (layer_relative * slice_pitch * r.image_extent.depth) as u64;
        let aligned_offset =
            layer_offset & !(d3d12::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64 - 1);
        if layer_offset == aligned_offset && is_pitch_aligned {
            // trivial case: everything is aligned, ready for copying
            copies.push(CopyRegion {
                footprint_offset: aligned_offset,
                footprint: image_extent_aligned,
                row_pitch,
                img_subresource,
                img_offset: r.image_offset,
                buf_offset: image::Offset::ZERO,
                copy_extent: image_extent_aligned,
            });
        } else if is_pitch_aligned {
            // buffer offset is not aligned
            let row_pitch_texels = row_pitch / image.bytes_per_block * image.block_dim.0;
            let gap = (layer_offset - aligned_offset) as i32;
            let buf_offset = image::Offset {
                x: (gap % row_pitch as i32) / image.bytes_per_block as i32
                    * image.block_dim.0 as i32,
                y: (gap % slice_pitch as i32) / row_pitch as i32 * image.block_dim.1 as i32,
                z: gap / slice_pitch as i32,
            };
            let footprint = image::Extent {
                width: buf_offset.x as u32 + image_extent_aligned.width,
                height: buf_offset.y as u32 + image_extent_aligned.height,
                depth: buf_offset.z as u32 + image_extent_aligned.depth,
            };
            if r.image_extent.width + buf_offset.x as u32 <= row_pitch_texels {
                // we can map it to the aligned one and adjust the offsets accordingly
                copies.push(CopyRegion {
                    footprint_offset: aligned_offset,
                    footprint,
                    row_pitch,
                    img_subresource,
                    img_offset: r.image_offset,
                    buf_offset,
                    copy_extent: image_extent_aligned,
                });
            } else {
                // split the copy region into 2 that suffice the previous condition
                assert!(buf_offset.x as u32 <= row_pitch_texels);
                let half = row_pitch_texels - buf_offset.x as u32;
                assert!(half <= r.image_extent.width);

                copies.push(CopyRegion {
                    footprint_offset: aligned_offset,
                    footprint: image::Extent {
                        width: row_pitch_texels,
                        ..footprint
                    },
                    row_pitch,
                    img_subresource,
                    img_offset: r.image_offset,
                    buf_offset,
                    copy_extent: image::Extent {
                        width: half,
                        ..r.image_extent
                    },
                });
                copies.push(CopyRegion {
                    footprint_offset: aligned_offset,
                    footprint: image::Extent {
                        width: image_extent_aligned.width - half,
                        height: footprint.height + image.block_dim.1 as u32,
                        depth: footprint.depth,
                    },
                    row_pitch,
                    img_subresource,
                    img_offset: image::Offset {
                        x: r.image_offset.x + half as i32,
                        ..r.image_offset
                    },
                    buf_offset: image::Offset {
                        x: 0,
                        y: buf_offset.y + image.block_dim.1 as i32,
                        z: buf_offset.z,
                    },
                    copy_extent: image::Extent {
                        width: image_extent_aligned.width - half,
                        ..image_extent_aligned
                    },
                });
            }
        } else {
            // worst case: row by row copy
            for z in 0..r.image_extent.depth {
                for y in 0..image_extent_aligned.height / image.block_dim.1 as u32 {
                    // an image row starts non-aligned
                    let row_offset =
                        layer_offset + z as u64 * slice_pitch as u64 + y as u64 * row_pitch as u64;
                    let aligned_offset =
                        row_offset & !(d3d12::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64 - 1);
                    let next_aligned_offset =
                        aligned_offset + d3d12::D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as u64;
                    let cut_row_texels = (next_aligned_offset - row_offset)
                        / image.bytes_per_block as u64
                        * image.block_dim.0 as u64;
                    let cut_width =
                        cmp::min(image_extent_aligned.width, cut_row_texels as image::Size);
                    let gap_texels = (row_offset - aligned_offset) as image::Size
                        / image.bytes_per_block as image::Size
                        * image.block_dim.0 as image::Size;
                    // this is a conservative row pitch that should be compatible with both copies
                    let max_unaligned_pitch =
                        (r.image_extent.width + gap_texels) * image.bytes_per_block;
                    let row_pitch =
                        (max_unaligned_pitch | (d3d12::D3D12_TEXTURE_DATA_PITCH_ALIGNMENT - 1)) + 1;

                    copies.push(CopyRegion {
                        footprint_offset: aligned_offset,
                        footprint: image::Extent {
                            width: cut_width + gap_texels,
                            height: image.block_dim.1 as _,
                            depth: 1,
                        },
                        row_pitch,
                        img_subresource,
                        img_offset: image::Offset {
                            x: r.image_offset.x,
                            y: r.image_offset.y + image.block_dim.1 as i32 * y as i32,
                            z: r.image_offset.z + z as i32,
                        },
                        buf_offset: image::Offset {
                            x: gap_texels as i32,
                            y: 0,
                            z: 0,
                        },
                        copy_extent: image::Extent {
                            width: cut_width,
                            height: image.block_dim.1 as _,
                            depth: 1,
                        },
                    });

                    // and if it crosses a pitch alignment - we copy the rest separately
                    if cut_width >= image_extent_aligned.width {
                        continue;
                    }
                    let leftover = image_extent_aligned.width - cut_width;

                    copies.push(CopyRegion {
                        footprint_offset: next_aligned_offset,
                        footprint: image::Extent {
                            width: leftover,
                            height: image.block_dim.1 as _,
                            depth: 1,
                        },
                        row_pitch,
                        img_subresource,
                        img_offset: image::Offset {
                            x: r.image_offset.x + cut_width as i32,
                            y: r.image_offset.y + y as i32 * image.block_dim.1 as i32,
                            z: r.image_offset.z + z as i32,
                        },
                        buf_offset: image::Offset::ZERO,
                        copy_extent: image::Extent {
                            width: leftover,
                            height: image.block_dim.1 as _,
                            depth: 1,
                        },
                    });
                }
            }
        }
    }
}
