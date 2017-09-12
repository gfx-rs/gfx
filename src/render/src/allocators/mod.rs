mod boxed;
mod stack;

pub use self::boxed::BoxedAllocator;
pub use self::stack::StackAllocator;

// TODO: fallbacks when out of memory
