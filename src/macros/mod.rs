#[macro_export]
macro_rules! gfx_vertex {
    ($name:ident {$($field:ident: $ty:ty,)*}) => {
        struct $name {
            $($field: $ty,)*
        }
        impl $crate::VertexFormat for $name {
            fn generate<R: $crate::Resources>(buffer: &$crate::handle::Buffer<R, $name>)
                        -> Vec<$crate::Attribute<R>> {
                use std::mem::size_of;
                use $crate::attrib::{Offset, Stride};
                use $crate::attrib::format::ToFormat;
                let stride = size_of::<$name>() as Stride;
                let mut offset = 0 as Offset;
                let mut attributes = Vec::new();
                $(
                    let (count, etype) = <$ty as ToFormat>::describe();
                    let format = $crate::attrib::Format {
                        elem_count: count,
                        elem_type: etype,
                        offset: offset,
                        stride: stride,
                        instance_rate: 0,
                    };
                    attributes.push($crate::Attribute {
                        name: String::new(), //fixme
                        format: format,
                        buffer: buffer.raw().clone(),
                    });
                    offset += size_of::<$ty>() as Offset;
                )*
                assert_eq!(offset, stride as Offset);
                attributes
            }
        }
    }
}

#[test]
fn vertex() {
    gfx_vertex!(_Foo {
        _x: i8,
        _y: f32,
        _z: [u32; 4],
    });
}
