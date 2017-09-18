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
            if node.start < node.end {
                self.free_list.push_back(node);
            }
            self.free_list.append(&mut tail);

            allocated
        })
    }

    pub fn deallocate(&mut self, mut range: Range<u64>) {
        // early out for invalid or empty ranges
        if range.end <= range.start {
            return;
        }

        // Find node where we want to insert the range.
        // We aim to merge consecutive nodes into larger ranges, so we maintain
        // a sorted list.
        let mut insert_index = self.free_list.len(); // append at the end
        for (index, node) in self.free_list.iter().enumerate() {
            if node.start > range.start {
                // Found a better place!
                insert_index = index;
                break;
            }
        }

        // New list: [head] -- [node] -- [tail]
        let mut tail = self.free_list.split_off(insert_index);

        // Try merge with prior node from [head]
        let pre_node = self.free_list.pop_back();
        pre_node.map(|pre_node| {
            if pre_node.end == range.start {
                // Merge both nodes
                range.start = pre_node.start;
            } else {
                // Re-insert the previous node
                self.free_list.push_back(pre_node);
            }
        });

        // Try merge with next node from [tail]
        let next_node = tail.pop_front();
        next_node.map(|next_node| {
            if range.end == next_node.start {
                // Merge both nodes
                range.end = next_node.end;
            } else {
                // Re-insert the next node
                tail.push_front(next_node);
            }
        });

        self.free_list.push_back(range);
        self.free_list.append(&mut tail);
    }
}

#[cfg(test)]
mod tests {
    use super::Allocator;

    #[test]
    fn test_allocate() {
        let mut allocator = Allocator::new(8);
        assert_eq!(Some(0..4), allocator.allocate(4));
        assert_eq!(Some(4..6), allocator.allocate(2));
        assert_eq!(Some(6..8), allocator.allocate(2));
        assert_eq!(None, allocator.allocate(1));
    }

    #[test]
    fn test_merge() {
        let mut allocator = Allocator::new(8);
        let front = allocator.allocate(4).unwrap();
        let middle = allocator.allocate(2).unwrap();
        let back = allocator.allocate(2).unwrap();

        allocator.deallocate(front);
        allocator.deallocate(back);

        // Fragmented, unable to allocate even though 6 elements would be available
        assert_eq!(None, allocator.allocate(5));
        allocator.deallocate(middle);
        assert_eq!(Some(0..5), allocator.allocate(5));
    }
}
