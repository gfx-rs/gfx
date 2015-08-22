// Copyright 2015 The Gfx-rs Developers.
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

use device as d;
use device::Resources;

struct Entry<R: Resources> {
    last_used: u16,
    bound: Option<(d::tex::Kind, R::Texture, Option<(R::Sampler, d::tex::SamplerInfo)>)>,
}

pub struct TextureCache<R: Resources> {
    count: u16,
    textures: Vec<Entry<R>>
}

fn age(now: u16, age: u16) -> u16 {
    use std::num::Wrapping;
    (Wrapping(now) - Wrapping(age)).0
}

impl<R> TextureCache<R> where R: Resources {
    /// Create a TextureCache with `x` slots
    pub fn new(slots: usize) -> TextureCache<R> {
        return TextureCache{
            count: 0,
            textures: (0..slots).map(|_| Entry {
                last_used: 0,
                bound: None
            }).collect()
        }
    }

    /// Returns the number of slots, If a draw call needs
    /// to bind more then this number of slots it will cause
    /// undefined behavior.
    pub fn number_of_slots(&self) -> usize {
        self.textures.len()
    }


    /// Bind a texture, this will look up the texture to see if
    /// it has been bound to a slot, if it has it will return that
    /// slot rather then sending a bind command, iff not it will 
    /// release a free slot and bind the texture to it. If there is
    /// no free slots, and there is no bound texture we will throw
    /// away the oldest entries first to make room for the new ones
    pub fn bind_texture<C>(&mut self,
                           kind: d::tex::Kind,
                           tex: R::Texture,
                           samp: Option<(R::Sampler, d::tex::SamplerInfo)>,
                           cb: &mut C) -> d::TextureSlot
        where C: d::draw::CommandBuffer<R>
    {
        self.count += 1;
        let count = self.count;

        let bind = (kind, tex, samp);
        for (i, ent) in self.textures.iter_mut().enumerate() {
            if let Some(ref bound) = ent.bound {
                if bound.0 == bind.0 && bound.1 == bind.1 && bound.2 == bind.2 {
                    // Update the LRU with the current count
                    ent.last_used = count;
                    return i as d::TextureSlot;
                }
            }
        }

        // No texture was found that was bound to the texture slot
        let mut oldest = 0;
        for i in (0..self.textures.len()) {
            if self.textures[i].bound.is_none() {
                oldest = i;
                break;
            }
            if age(count, self.textures[i].last_used) > age(count, self.textures[oldest].last_used) {
                oldest = i;
            }
        }

        cb.bind_texture(oldest as d::TextureSlot, bind.0, bind.1, bind.2);

        self.textures[oldest].last_used = count;
        self.textures[oldest].bound = Some(bind);
        return oldest as d::TextureSlot;
    }

    /// Clear the texture cache
    pub fn clear(&mut self) {
        self.count = 0;
        for ent in self.textures.iter_mut() {
            ent.last_used = 0;
            ent.bound = None;
        }
    }
}

#[test]
fn test_age() {
    assert_eq!(age(100, 0), 100);
    assert_eq!(age(0, 0), 0);
    assert_eq!(age(0, 0xFFFF), 1);
    assert_eq!(age(0, 0xFFFE), 2);
}
