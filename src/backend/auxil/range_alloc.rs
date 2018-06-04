use std::fmt::Debug;
use std::ops::{Add, AddAssign, Range, Sub};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct RangeAllocator<T> {
    /// The range this allocator covers.
    initial_range: Range<T>,
    /// A Vec of ranges in this heap which are unused.
    /// Must be ordered with ascending range start to permit short circuiting allocation.
    /// No two ranges in this vec may overlap.
    free_ranges: Vec<Range<T>>,
}

impl<T> RangeAllocator<T>
where
    T: Clone + Copy + Add<Output = T> + AddAssign + Sub<Output = T> + Eq + PartialOrd + Debug,
{
    pub fn new(range: Range<T>) -> Self {
        RangeAllocator {
            initial_range: range.clone(),
            free_ranges: vec![range.clone()],
        }
    }

    pub fn allocate_range(&mut self, length: T) -> Option<Range<T>> {
        let mut best_fit: Option<(usize, Range<T>)> = None;
        for (index, range) in self.free_ranges.iter().cloned().enumerate() {
            let range_length = range.end - range.start;
            if range_length < length {
                continue;
            } else if range_length == length {
                // Found a perfect fit, so stop looking.
                best_fit = Some((index, range));
                break;
            }
            best_fit = Some(match best_fit {
                Some((best_index, best_range)) => {
                    // Find best fit for this allocation to reduce memory fragmentation.
                    if range_length < best_range.end - best_range.start {
                        (index, range)
                    } else {
                        (best_index, best_range.clone())
                    }
                }
                None => {
                    (index, range)
                }
            });
        }
        best_fit.map(|(index, range)| {
            if range.end - range.start == length {
                self.free_ranges.remove(index);
            } else {
                self.free_ranges[index].start += length;
            }
            range.start..(range.start + length)
        })
    }

    pub fn free_range(&mut self, range: Range<T>) {
        assert!(self.initial_range.start <= range.start && range.end <= self.initial_range.end);
        assert!(range.start < range.end);

        // Get insertion position.
        let i = self.free_ranges.iter()
            .position(|r| r.start > range.start)
            .unwrap_or(self.free_ranges.len());

        // Try merging with neighboring ranges in the free list.
        // Before: |left|-(range)-|right|
        if i > 0 && range.start == self.free_ranges[i - 1].end {
            // Merge with |left|.
            self.free_ranges[i - 1].end =
                if i < self.free_ranges.len() && range.end == self.free_ranges[i].start {
                    // Check for possible merge with |left| and |right|.
                    let right = self.free_ranges.remove(i);
                    right.end
                } else {
                    range.end
                };

            return;
        } else if i < self.free_ranges.len() && range.end == self.free_ranges[i].start {
            // Merge with |right|.
            self.free_ranges[i].start =
                if i > 0 && range.start == self.free_ranges[i - 1].end {
                    // Check for possible merge with |left| and |right|.
                    let left = self.free_ranges.remove(i - 1);
                    left.start
                } else {
                    range.start
                };

            return;
        }

        // Debug checks
        assert!(
            (i == 0 || self.free_ranges[i - 1].end < range.start) &&
            (i >= self.free_ranges.len() || range.end < self.free_ranges[i].start)
        );

        self.free_ranges.insert(i, range);
    }

    pub fn reset(&mut self) {
        self.free_ranges.clear();
        self.free_ranges.push(self.initial_range.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_allocation() {
        let mut alloc = RangeAllocator::new(0..10);
        // Test if an allocation works
        assert_eq!(alloc.allocate_range(4), Some(0..4));
        // Free the prior allocation
        alloc.free_range(0..4);
        // Make sure the free actually worked
        assert_eq!(alloc.free_ranges, vec![0..10]);
    }

    #[test]
    fn test_out_of_space() {
        let mut alloc = RangeAllocator::new(0..10);
        // Test if the allocator runs out of space correctly
        assert_eq!(alloc.allocate_range(10), Some(0..10));
        assert!(alloc.allocate_range(4).is_none());
        alloc.free_range(0..10);
    }

    #[test]
    fn test_dont_use_block_that_is_too_small() {
        let mut alloc = RangeAllocator::new(0..10);
        // Allocate three blocks then free the middle one and check for correct state
        assert_eq!(alloc.allocate_range(3), Some(0..3));
        assert_eq!(alloc.allocate_range(3), Some(3..6));
        assert_eq!(alloc.allocate_range(3), Some(6..9));
        alloc.free_range(3..6);
        assert_eq!(alloc.free_ranges, vec![3..6, 9..10]);
        // Now request space that the middle block can fill, but the end one can't.
        assert_eq!(alloc.allocate_range(3), Some(3..6));
    }

    #[test]
    fn test_free_blocks_in_middle() {
        let mut alloc = RangeAllocator::new(0..100);
        // Allocate many blocks then free every other block.
        assert_eq!(alloc.allocate_range(10), Some(0..10));
        assert_eq!(alloc.allocate_range(10), Some(10..20));
        assert_eq!(alloc.allocate_range(10), Some(20..30));
        assert_eq!(alloc.allocate_range(10), Some(30..40));
        assert_eq!(alloc.allocate_range(10), Some(40..50));
        assert_eq!(alloc.allocate_range(10), Some(50..60));
        assert_eq!(alloc.allocate_range(10), Some(60..70));
        assert_eq!(alloc.allocate_range(10), Some(70..80));
        assert_eq!(alloc.allocate_range(10), Some(80..90));
        assert_eq!(alloc.allocate_range(10), Some(90..100));
        assert_eq!(alloc.free_ranges, vec![]);
        alloc.free_range(10..20);
        alloc.free_range(30..40);
        alloc.free_range(50..60);
        alloc.free_range(70..80);
        alloc.free_range(90..100);
        // Check that the right blocks were freed.
        assert_eq!(alloc.free_ranges, vec![10..20, 30..40, 50..60, 70..80, 90..100]);
        // Fragment the memory on purpose a bit.
        assert_eq!(alloc.allocate_range(6), Some(10..16));
        assert_eq!(alloc.allocate_range(6), Some(30..36));
        assert_eq!(alloc.allocate_range(6), Some(50..56));
        assert_eq!(alloc.allocate_range(6), Some(70..76));
        assert_eq!(alloc.allocate_range(6), Some(90..96));
        // Check for fragmentation.
        assert_eq!(alloc.free_ranges, vec![16..20, 36..40, 56..60, 76..80, 96..100]);
        // Fill up the fragmentation
        assert_eq!(alloc.allocate_range(4), Some(16..20));
        assert_eq!(alloc.allocate_range(4), Some(36..40));
        assert_eq!(alloc.allocate_range(4), Some(56..60));
        assert_eq!(alloc.allocate_range(4), Some(76..80));
        assert_eq!(alloc.allocate_range(4), Some(96..100));
        // Check that nothing is free.
        assert_eq!(alloc.free_ranges, vec![]);
    }

    #[test]
    fn test_ignore_block_if_another_fits_better() {
        let mut alloc = RangeAllocator::new(0..10);
        // Allocate blocks such that the only free spaces available are 3..6 and 9..10
        // in order to prepare for the next test.
        assert_eq!(alloc.allocate_range(3), Some(0..3));
        assert_eq!(alloc.allocate_range(3), Some(3..6));
        assert_eq!(alloc.allocate_range(3), Some(6..9));
        alloc.free_range(3..6);
        assert_eq!(alloc.free_ranges, vec![3..6, 9..10]);
        // Now request space that can be filled by 3..6 but should be filled by 9..10
        // because 9..10 is a perfect fit.
        assert_eq!(alloc.allocate_range(1), Some(9..10));
    }

    #[test]
    fn test_merge_neighbors() {
        let mut alloc = RangeAllocator::new(0..9);
        assert_eq!(alloc.allocate_range(3), Some(0..3));
        assert_eq!(alloc.allocate_range(3), Some(3..6));
        assert_eq!(alloc.allocate_range(3), Some(6..9));
        alloc.free_range(0..3);
        alloc.free_range(6..9);
        alloc.free_range(3..6);
        assert_eq!(alloc.free_ranges, vec![0..9]);
    }
}
