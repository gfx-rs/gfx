//! Functionality only required for backend implementations.

/// Fast hash map used internally.
#[cfg(feature = "fxhash")]
pub type FastHashMap<K, V> = std::collections::HashMap<K, V, std::hash::BuildHasherDefault<fxhash::FxHasher>>;
