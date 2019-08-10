//! Functionality only required for backend implementations.

use std::collections::HashMap;
use std::hash::BuildHasherDefault;

use fxhash::FxHasher;

/// Fast hash map used internally.
pub type FastHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
