//! Free-list allocator for descriptor heaps.

use std::collections::LinkedList;
use std::ops::Range;

#[derive(Debug)]
pub struct Allocator {
    size: u64,
    free_list: LinkedList<Range<u64>>,
}

impl Allocator {
    pub fn new(size: u64) -> Self {
        // Node spanning the whole heap.
        let node = Range {
            start: 0,
            end: size,
        };
        let mut free_list = LinkedList::new();
        free_list.push_front(node);
        Allocator {
            size,
            free_list,
        }
    }

    pub fn allocate(&mut self, size: u64) -> Option<Range<u64>> {
        if size == 0 {
            return Some(Range { start: 0, end: 0 });
        }

        // Find first node which is big enough.
        let mut split_index = None;
        for (index, node) in self.free_list.iter().enumerate() {
            if node.end >= node.start + size {
                // Found a candidate.
                split_index = Some(index);
                break;
            }
        }

        split_index.map(|index| {
            let mut tail = self.free_list.split_off(index);

            // The first list element of `tail` will be split into two nodes.
            let mut node = tail.pop_front().unwrap();
            let allocated = Range {
                start: node.start,
                end: node.start + size,
            };
            node.start += size;

            // Our new list will look like this considering our 2nd node part
            // is not empty:
            // Before: [old list] -- [allocated|node] -- [tail]
            // After:  [old list] -- [node] -- [tail] || [allocated]
            if node.start >= node.end {
                self.free_list.push_back(node);
            }
            self.free_list.append(&mut tail);

            allocated
        })
    }
}

