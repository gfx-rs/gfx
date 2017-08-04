// Copyright 2017 The Gfx-rs Developers.
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

// TODO: move this into render in the long-term

use {handle};
use {Backend, SubmissionError, SubmissionResult};

use std::collections::hash_set::{self, HashSet};
use std::ops::Deref;

/// Informations about what is accessed by a bunch of commands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessInfo<B: Backend> {
    mapped_reads: HashSet<handle::RawBuffer<B>>,
    mapped_writes: HashSet<handle::RawBuffer<B>>,
}

impl<B: Backend> AccessInfo<B> {
    /// Creates empty access informations
    pub fn new() -> Self {
        AccessInfo {
            mapped_reads: HashSet::new(),
            mapped_writes: HashSet::new(),
        }
    }

    /// Clear access informations
    pub fn clear(&mut self) {
        self.mapped_reads.clear();
        self.mapped_writes.clear();
    }

    /// Register a buffer read access
    pub fn buffer_read(&mut self, buffer: &handle::RawBuffer<B>) {
        if buffer.is_mapped() {
            self.mapped_reads.insert(buffer.clone());
        }
    }

    /// Register a buffer write access
    pub fn buffer_write(&mut self, buffer: &handle::RawBuffer<B>) {
        if buffer.is_mapped() {
            self.mapped_writes.insert(buffer.clone());
        }
    }

    /// Returns the mapped buffers that The GPU will read from
    pub fn mapped_reads(&self) -> AccessInfoBuffers<B> {
        self.mapped_reads.iter()
    }

    /// Returns the mapped buffers that The GPU will write to
    pub fn mapped_writes(&self) -> AccessInfoBuffers<B> {
        self.mapped_writes.iter()
    }

    /// Is there any mapped buffer reads ?
    pub fn has_mapped_reads(&self) -> bool {
        !self.mapped_reads.is_empty()
    }

    /// Is there any mapped buffer writes ?
    pub fn has_mapped_writes(&self) -> bool {
        !self.mapped_writes.is_empty()
    }

    /// Takes all the accesses necessary for submission
    pub fn take_accesses(&self) -> SubmissionResult<AccessGuard<B>> {
        for buffer in self.mapped_reads().chain(self.mapped_writes()) {
            unsafe {
                if !buffer.mapping().unwrap().take_access() {
                    return Err(SubmissionError::AccessOverlap);
                }
            }
        }
        Ok(AccessGuard { inner: self })
    }
}

#[allow(missing_docs)]
pub type AccessInfoBuffers<'a, B> = hash_set::Iter<'a, handle::RawBuffer<B>>;

#[allow(missing_docs)]
#[derive(Debug)]
pub struct AccessGuard<'a, B: Backend> {
    inner: &'a AccessInfo<B>,
}

#[allow(missing_docs)]
impl<'a, B: Backend> AccessGuard<'a, B> {
    /// Returns the mapped buffers that The GPU will read from,
    /// with exclusive acces to their mapping
    pub fn access_mapped_reads(&mut self) -> AccessGuardBuffers<B> {
        AccessGuardBuffers {
            buffers: self.inner.mapped_reads()
        }
    }

    /// Returns the mapped buffers that The GPU will write to,
    /// with exclusive acces to their mapping
    pub fn access_mapped_writes(&mut self) -> AccessGuardBuffers<B> {
        AccessGuardBuffers {
            buffers: self.inner.mapped_writes()
        }
    }

    pub fn access_mapped(&mut self) -> AccessGuardBuffersChain<B> {
        AccessGuardBuffersChain {
            fst: self.inner.mapped_reads(),
            snd: self.inner.mapped_writes(),
        }
    }
}

impl<'a, B: Backend> Deref for AccessGuard<'a, B> {
    type Target = AccessInfo<B>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, B: Backend> Drop for AccessGuard<'a, B> {
    fn drop(&mut self) {
        for buffer in self.inner.mapped_reads().chain(self.inner.mapped_writes()) {
            unsafe {
                buffer.mapping().unwrap().release_access();
            }
        }
    }
}

#[allow(missing_docs)]
#[derive(Debug)]
pub struct AccessGuardBuffers<'a, B: Backend> {
    buffers: AccessInfoBuffers<'a, B>
}

impl<'a, B: Backend> Iterator for AccessGuardBuffers<'a, B> {
    type Item = (&'a handle::RawBuffer<B>, &'a mut B::Mapping);

    fn next(&mut self) -> Option<Self::Item> {
        self.buffers.next().map(|buffer| unsafe {
            (buffer, buffer.mapping().unwrap().use_access())
        })
    }
}

#[allow(missing_docs)]
#[derive(Debug)]
pub struct AccessGuardBuffersChain<'a, B: Backend> {
    fst: AccessInfoBuffers<'a, B>,
    snd: AccessInfoBuffers<'a, B>
}

impl<'a, B: Backend> Iterator for AccessGuardBuffersChain<'a, B> {
    type Item = (&'a handle::RawBuffer<B>, &'a mut B::Mapping);

    fn next(&mut self) -> Option<Self::Item> {
        self.fst.next().or_else(|| self.snd.next())
            .map(|buffer| unsafe {
                (buffer, buffer.mapping().unwrap().use_access())
            })
    }
}
