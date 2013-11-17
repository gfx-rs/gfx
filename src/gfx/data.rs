// Copyright 2013 The Gfx-rs Developers.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[macro_escape];

//! The `data_manager!` macro is used to generate a data manager for a specific
//! set of fields. The data is stored in an 'unzipped' format, allowing the
//! operations to be optimised towards batch operations, at the expense
//! of direct handle lookups and element addition and removal.
//!
//! ~~~rust
//! pub mod $manager_mod {
//!     pub struct Handle;
//!     pub struct Manager;
//!
//!     pub struct DataRef<'a> { $($field: &'a $Field     ),* }
//!     pub struct DataMut<'a> { $($field: &'a mut $Field ),* }
//!
//!     pub struct DataRefIterator<'a>;
//!     pub struct DataMutIterator<'a>;
//!     pub type DataRefRevIterator<'a>;
//!     pub type DataMutRevIterator<'a>;
//! }
//! ~~~
//!
//! ~~~
//!                                         +---+
//!                                         |   |  Handle
//!                                         +-.-+
//!                                           |
//! +- Manager -------------------------------|-----------------+
//! |                                         |                 |
//! |   +---+---+---+---+---+---+---+---+---+-V-+ - - - +---+   |
//! |   | i | i |   | i | i | i | i |       | i |           |   |
//! |   +-.-+-.-+---+-.-+-.-+-.-+-.-+---+---+-.-+ - - - +---+   |
//! |     |   |       |   |   |   |           |                 |
//! |     |   |   +---|---+   |   |           |                 |
//! |     |   |   |   |       |   |           |                 |
//! |     |   |   |   |   +---|---|-----------+                 |
//! |     |   |   |   |   |   |   |                             |
//! |   +-V-+-V-+-V-+-V-+-V-+-V-+-V-+---+---+---+ - - - +---+   |
//! |   | x | x | x | x | x | x | x |                       |   |
//! |   +---+---+---+---+---+---+---+---+---+---+ - - - +---+   |
//! |   | x | x | x | x | x | x | x |                       |   |
//! |   +---+---+---+---+---+---+---+---+---+---+ - - - +---+   |
//! |   :   :   :   :   :   :   :   :                       :   |
//! |   +---+---+---+---+---+---+---+---+---+---+ - - - +---+   |
//! |   | x | x | x | x | x | x | x |                       |   |
//! |   +---+---+---+---+---+---+---+---+---+---+ - - - +---+   |
//! |                                                           |
//! +-----------------------------------------------------------+
//! ~~~

macro_rules! data_manager(
    (data $manager_mod:ident {
        $($field:ident : $Field:ty),*
    }) => {
        pub mod $manager_mod {
            use std::iter::Invert;
            use std::u16;
            use std::vec;

            /// A handle to a data element holding a type
            #[deriving(Clone, Eq)]
            pub struct Handle {
                /// The index of the data reference associated with the handle
                priv ref_index: u16,
                /// The generation of the index, used for checking if the handle is invalid
                priv generation: u16,
            }

            #[deriving(Clone, Eq)]
            struct InnerRef {
                /// The index of the data
                data_index: Option<u16>,
                /// The generation of the index
                generation: u16,
            }

            pub struct DataRef<'a> {
                $($field: &'a $Field,)*
            }

            pub struct DataMut<'a> {
                $($field: &'a mut $Field,)*
            }

            pub struct Manager {
                priv inner_refs: ~[InnerRef],
                $(priv $field: ~[$Field],)*
                priv capacity: u16,
                priv len: u16,
            }

            impl Manager {
                pub fn new() -> Manager {
                    Manager::new_sized(u16::max_value)
                }

                pub fn new_sized(capacity: u16) -> Manager {
                    Manager {
                        inner_refs: vec::from_elem(
                            capacity as uint,
                            InnerRef {
                                data_index: None,
                                generation: 0,
                            }
                        ),
                        $($field: vec::with_capacity(capacity as uint),)*
                        capacity: capacity,
                        len: 0,
                    }
                }

                #[inline]
                pub fn capacity(&self) -> u16 {
                    self.capacity
                }

                #[inline]
                pub fn len(&self) -> u16 {
                    self.len
                }

                #[inline]
                pub fn is_empty(&self) -> bool {
                    self.len == 0
                }

                #[inline]
                pub fn add(&mut self, $($field: $Field),*) -> Handle {
                    self.add_opt($($field),*)
                        .expect(~"No room to store the data.") // TODO: better error message?
                }

                pub fn add_opt(&mut self, $($field: $Field),*) -> Option<Handle> {
                    let new_len = match self.len.checked_add(&1) {
                        Some(new_len) => new_len, _ => return None,
                    };
                    match self.inner_refs.mut_iter().position(|i| i.data_index == None) {
                        Some(i) if new_len <= self.capacity => { // TODO: is this check really necessary?
                            // Store the index to the data
                            self.inner_refs[i].data_index = Some(self.len);
                            self.len = new_len;
                            // Add the field data
                            $(self.$field.push($field);)*
                            // Return a handle to the stored data
                            Some(Handle {
                                ref_index: i as u16,
                                generation: self.inner_refs[i].generation
                            })
                        }
                        _ => None,
                    }
                }

                // pub fn remove(&mut self, handle: Handle) -> bool {
                //     let hi = handle.index as uint;
                //     let iref = &mut self.inner_refs[hi];

                //     if handle.generation >= iref.generation {
                //         return false;
                //     }

                //     let di = iref.index.unwrap();

                //     // Remove the reference to the element from the indices and clear
                //     // it from the vector of elements
                //     iref.index = None;
                //     iref.count = iref.count.checked_add(&1)
                //                      .expect("The maximum age of the element was reached.");

                //     match self.len.checked_sub(&1) {
                //         Some(lasti) =>
                //             self.$field0.swap_remove(di);
                //             $(self.$field.swap_remove(di);)*

                //             for iref in self.inner_refs.mut_iter() {
                //                 if iref.data_index == Some(lasti) {
                //                     iref.data_index = Some(di);
                //                 }
                //             }
                //             self.len = lasti;
                //             true
                //         }
                //         _ => false,
                //     }
                // }

                #[inline]
                fn get_data_index(&self, handle: Handle) -> u16 {
                    let h = &self.inner_refs[handle.ref_index as uint];
                    assert!(handle.generation >= h.generation);
                    h.data_index.expect("") // TODO: error message
                }

                #[inline]
                pub fn get<'a>(&'a self, handle: Handle) -> DataRef<'a> {
                    let i = self.get_data_index(handle) as uint;
                    DataRef {
                        $($field: &'a self.$field[i]),*
                    }
                }

                #[inline]
                pub fn get_mut<'a>(&'a mut self, handle: Handle) -> DataMut<'a> {
                    let i = self.get_data_index(handle) as uint;
                    DataMut {
                        $($field: &'a mut self.$field[i]),*
                    }
                }

                #[inline]
                pub fn find<'a>(&'a self, handle: Handle) -> Option<DataRef<'a>> {
                    match self.inner_refs[handle.ref_index as uint] {
                        InnerRef { data_index: Some(i), generation: c } if c <= handle.generation => {
                            Some(DataRef { $($field: &'a self.$field[i]),* })
                        }
                        _ => None,
                    }
                }

                #[inline]
                pub fn find_mut<'a>(&'a mut self, handle: Handle) -> Option<DataMut<'a>> {
                    match self.inner_refs[handle.ref_index as uint] {
                        InnerRef { data_index: Some(i), generation: c } if c <= handle.generation => {
                            Some(DataMut { $($field: &'a mut self.$field[i]),* })
                        }
                        _ => None,
                    }
                }

                #[inline]
                pub fn set<'a>(&'a mut self, handle: Handle, $($field: $Field),*) {
                    let i = self.get_data_index(handle) as uint;
                    $(self.$field[i] = $field;)*
                }


                #[inline]
                pub fn iter<'a>(&'a self) -> DataRefIterator<'a> {
                    fail!("Not yet implemented.")
                }

                #[inline]
                pub fn mut_iter<'a>(&'a mut self) -> DataMutIterator<'a> {
                    fail!("Not yet implemented.")
                }

                #[inline]
                pub fn rev_iter<'a>(&'a self) -> DataRefRevIterator<'a> {
                    self.iter().invert()
                }

                #[inline]
                fn mut_rev_iter<'a>(&'a mut self) -> DataMutRevIterator<'a> {
                    self.mut_iter().invert()
                }
            }

            pub struct DataRefIterator<'a>;
            pub struct DataMutIterator<'a>;
            
            impl<'a> Iterator<DataRef<'a>> for DataRefIterator<'a> {
                fn next(&mut self) -> Option<DataRef<'a>> { fail!("Not yet implemented.") }
                fn size_hint(&self) -> (uint, Option<uint>) { fail!("Not yet implemented.") }
            }

            impl<'a> Iterator<DataMut<'a>> for DataMutIterator<'a> {
                fn next(&mut self) -> Option<DataMut<'a>> { fail!("Not yet implemented.") }
                fn size_hint(&self) -> (uint, Option<uint>) { fail!("Not yet implemented.") }
            }

            impl<'a> DoubleEndedIterator<DataMut<'a>> for DataMutIterator<'a> {
                fn next_back(&mut self) -> Option<DataMut<'a>> { fail!("Not yet implemented.") }
            }

            impl<'a> DoubleEndedIterator<DataRef<'a>> for DataRefIterator<'a> {
                fn next_back(&mut self) -> Option<DataRef<'a>> { fail!("Not yet implemented.") }
            }

            impl<'a> ExactSize<DataRef<'a>> for DataRefIterator<'a> {}
            impl<'a> ExactSize<DataMut<'a>> for DataMutIterator<'a> {}

            pub type DataRefRevIterator<'a> = Invert<DataRefIterator<'a>>;
            pub type DataMutRevIterator<'a> = Invert<DataMutIterator<'a>>;
        }
    }
)

#[cfg(test)]
mod tests {
    use std::u16;

    #[deriving(Eq)]
    struct Point { x: int, y: int }

    data_manager! {
        data Animal {
            name: ~str,
            pos: super::Point,
        }
    }

    #[test]
    fn test_add() {
        let mut animals = Animal::Manager::new();
        let h = animals.add(~"kitten", Point { x: 2, y: 3 });
        assert!(animals.find(h).is_some());
    }

    #[test]
    #[should_fail]
    fn test_add_overflow() {
        let mut animals = Animal::Manager::new_sized(4);
        let _ = animals.add(~"kitten", Point { x: 2, y: 3 });
        let _ = animals.add(~"sloth", Point { x: 1, y: 6 });
        let _ = animals.add(~"puffin", Point { x: 8, y: 7 });
        let _ = animals.add(~"lobster", Point { x: 0, y: 5 });
        let _ = animals.add(~"whale", Point { x: 3, y: 1 });
    }

    #[test]
    fn test_remove_handle() {
        fail!("Not yet implemented.")
    }

    #[test]
    #[should_fail]
    fn test_remove_invalid_handle() {
        // fail!("Not yet implemented.")
    }

    #[test]
    fn test_capacity() {
        assert_eq!(Animal::Manager::new().capacity(), u16::max_value);
        assert_eq!(Animal::Manager::new_sized(5).capacity(), 5);
    }

    #[test]
    fn test_len() {
        let mut animals = Animal::Manager::new();
        let _ = animals.add(~"kitten", Point { x: 2, y: 3 });
        let _ = animals.add(~"sloth", Point { x: 1, y: 6 });
        let _ = animals.add(~"puffin", Point { x: 8, y: 7 });
        assert_eq!(animals.len(), 3);
    }

    #[test]
    fn test_is_empty() {
        let mut animals = Animal::Manager::new();
        assert!(animals.is_empty());
        let _ = animals.add(~"kitten", Point { x: 2, y: 3 });
        assert!(!animals.is_empty());
    }

    #[test]
    fn test_get() {
        let mut animals = Animal::Manager::new();
        let h = animals.add(~"kitten", Point { x: 2, y: 3 });
        let x = animals.get(h);
        assert_eq!(*x.name, ~"kitten");
        assert_eq!(*x.pos, Point { x: 2, y: 3 });
    }

    #[test]
    #[should_fail]
    fn test_get_invalid() {
        // fail!("Not yet implemented.")
    }

    #[test]
    fn test_get_mut() {
        let mut animals = Animal::Manager::new();
        let h = animals.add(~"kitten", Point { x: 2, y: 3 });
        {
            let x = animals.get_mut(h);
            *x.name = ~"puppy";
            *x.pos = Point { x: 0, y: -1 };
        }
        let y = animals.get(h);
        assert_eq!(*y.name, ~"puppy");
        assert_eq!(*y.pos, Point { x: 0, y: -1 });
    }

    #[test]
    #[should_fail]
    fn test_get_mut_invalid() {
        // fail!("Not yet implemented.")
    }

    #[test]
    fn test_set() {
        let mut animals = Animal::Manager::new();
        let h = animals.add(~"kitten", Point { x: 2, y: 3 });
        animals.set(h, ~"puppy", Point { x: 0, y: -1 });
        let y = animals.get(h);
        assert_eq!(*y.name, ~"puppy");
        assert_eq!(*y.pos, Point { x: 0, y: -1 });
    }

    #[test]
    fn test_set_invalid() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_with() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_with_invalid() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_with_mut() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_with_mut_invalid() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_find() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_find_invalid() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_find_mut() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_find_mut_invalid() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_iter() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_rev_iter() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_mut_iter() {
        fail!("Not yet implemented.")
    }

    #[test]
    fn test_mut_rev_iter() {
        fail!("Not yet implemented.")
    }
}
