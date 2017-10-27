//! Various helper macros.

#[cfg_attr(test, macro_use)]
mod descriptors;
#[cfg_attr(test, macro_use)]
mod pipeline;
#[cfg_attr(test, macro_use)]
mod structure;
#[cfg(test)]
mod test;

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
