//! Various helper macros.

mod descriptors;
mod pipeline;

#[macro_export]
macro_rules! gfx_format {
    ($name:ident : $surface:ident = $container:ident<$channel:ident>) => {
        impl $crate::format::Formatted for $name {
            type Surface = $crate::format::$surface;
            type Channel = $crate::format::$channel;
            type View = $crate::format::$container<
                <$crate::format::$channel as $crate::format::ChannelTyped>::ShaderType
                >;
        }
    }
}

#[macro_export]
macro_rules! gfx_buffer_struct {
    ($name:ident { $( $field:ident: $ty:ty, )* }) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        #[allow(non_snake_case)]
        pub struct $name {
            $( $field: $ty, )*
        }

        unsafe impl $crate::memory::Pod for $name {}

        impl $crate::pso::Structure for $name
            where $( $ty: $crate::format::BufferFormat, )*
        {
            fn elements() -> Vec<$crate::core::pso::Element<$crate::format::Format>> {
                let mut elements = Vec::new();
                let mut offset = 0;
                $(
                    elements.push($crate::core::pso::Element {
                        format: <$ty as $crate::format::Formatted>::get_format(),
                        offset: offset as u32,
                    });
                    offset += ::std::mem::size_of::<$ty>();
                )*
                let _ = offset;
                elements
            }
        }
    }
}
