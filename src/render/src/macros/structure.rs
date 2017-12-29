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
            where $( $ty: $crate::format::AsFormat, )*
        {
            fn elements() -> Vec<$crate::hal::pso::Element<$crate::format::Format>> {
                let mut elements = Vec::new();
                let mut offset = 0;
                $(
                    elements.push($crate::hal::pso::Element {
                        format: <$ty as $crate::format::AsFormat>::SELF,
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
