// Copyright 2014 The Gfx-rs Developers.
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


//*****************************************************************************
// THIS IS A HUGE HACK AND WE WANT TO USE std::sync::Arc
// this will be removed once `is_unqiue` or something similar is stabilized.
//*****************************************************************************


use std::sync::atomic::*;
use std::ops::Deref;
use std::mem;
use std::fmt;
use std::hash;
use std::cmp;

struct Inner<T> {
    refs: AtomicUsize,
    data: T
}

pub struct Arc<T>(*const Inner<T>);

unsafe impl<T:Send+Sync> Send for Arc<T> {}
unsafe impl<T:Send+Sync> Sync for Arc<T> {}

impl<T:Send+Sync> Arc<T> {
    /// Create a new Arc<T>
    pub fn new(data: T) -> Arc<T> {
        let inner = Box::new(Inner {
            refs: AtomicUsize::new(1),
            data: data
        });

        unsafe {
            Arc(mem::transmute(inner))
        }
    }

    /// Like Ord, but checks by ref vs value
    pub fn cmp_ref(&self, rhs: &Arc<T>) -> cmp::Ordering {
        (self.0 as usize).cmp(&(rhs.0 as usize))
    }
}

impl<T> Arc<T> {
    /// Check to see if this is unique
    pub fn is_unique(&mut self) -> Option<&T> {
        unsafe {
            if 1 == (*self.0).refs.load(Ordering::SeqCst) {
                Some(&(*self.0).data)
            } else {
                None
            }
        }
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        unsafe {
            (*self.0).refs.fetch_add(1, Ordering::SeqCst);
        }
        Arc(self.0)
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe {
            &(*self.0).data
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Arc<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.deref().fmt(f)
    }
}

impl<T: PartialEq> PartialEq for Arc<T> {
    fn eq(&self, other: &Arc<T>) -> bool {
        self.deref() == other.deref()
    }
}

impl<T: Eq> Eq for Arc<T> {}

impl<T: hash::Hash> hash::Hash for Arc<T> {
    fn hash<H>(&self, state: &mut H) where H: hash::Hasher {
        self.deref().hash(state)
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        unsafe {
            if 1 == (*self.0).refs.fetch_sub(1, Ordering::SeqCst) {
                let _: Box<Inner<T>> = mem::transmute(self.0);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::Arc;
    use std::sync::Arc as StdArc;
    use std::sync::atomic::*;

    struct Canary(StdArc<AtomicUsize>);

    impl Drop for Canary {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn testing_drop() {
        let inner = StdArc::new(AtomicUsize::new(0));
        let data = Arc::new(Canary(inner.clone()));

        assert_eq!(0, inner.load(Ordering::SeqCst));
        drop(data.clone());
        assert_eq!(0, inner.load(Ordering::SeqCst));
        drop(data);
        assert_eq!(1, inner.load(Ordering::SeqCst));
    }

    #[test]
    fn testing_unique() {
        let mut data = Arc::new(0);
        assert!(data.is_unique().is_some());
        let mut data2 = data.clone();
        assert!(data.is_unique().is_none());
        assert!(data2.is_unique().is_none());
        drop(data);
        assert!(data2.is_unique().is_some());
    }
}