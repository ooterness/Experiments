/// Testing bitonic sorting networks for stability
/// Copyright 2021 by Alex Utter
///
/// This file defines a series of bitonic sorting networks, tests whether
/// they function correctly, and then tests whether their order remains
/// stable for tiebreaking purposes.
///
/// The motivation is an FPGA design problem described here:
/// https://www.reddit.com/r/FPGA/comments/qe9j6s/vectorpacking_algorithm/

use std::cmp;
use std::fmt;

// Parameters for creating a new Lane or LaneArray object
// (i.e., Options for how to initialize the key-values for sorting.)
enum LaneArrayType {
    Simple(u64),    // Key includes keep/discard mask and lane index
    Hidden(u64),    // Key includes keep/discard mask only
}

// Each "lane" has a key-value (for sorting) and metadata (for verification).
#[derive(Clone, Copy)]
struct Lane {
    key: u64,
    meta: u64,
}

// Use a large penalty to increment the keys of disabled lanes.
const PENALTY:u64 = 256;

impl Lane {
    // Create a key-value pair based on an index and mask.
    // Note: Verification data in unused lanes is "don't-care".
    fn new(typ:&LaneArrayType, idx:u8) -> Lane {
        let chk = 1u64 << idx;
        let idx64 = idx as u64;
        match typ {
            LaneArrayType::Simple(mask) => {
                let pen = if mask & chk > 0 {PENALTY} else {0};
                Lane {key: cmp::max(idx64,pen), meta: cmp::max(idx64,pen)}},
            LaneArrayType::Hidden(mask) => {
                let pen = if mask & chk > 0 {PENALTY} else {0};
                Lane {key: pen, meta: cmp::max(idx64,pen)}},
        }
    }
}

// A lane-swap operation is a pair of input/output indices.
// Order is preserved if #1.key <= #2.key, otherwise swap.
struct LaneSwap(usize, usize);

fn sw(a:usize, b:usize) -> LaneSwap {
    LaneSwap {0:a, 1:b}
}

// An array of lane values, which can be used as an input vector,
// the state of a pipeline stage, or a vector of outputs.
#[derive(Clone)]
struct LaneArray {
    lanes: Vec<Lane>,
}

impl LaneArray {
    // Create a new vector of Lanes of the designated size and type.
    fn new(len:u8, typ:&LaneArrayType) -> LaneArray {
        LaneArray {lanes: (0..len).map(|n| Lane::new(typ, n)).collect()}
    }

    // Are all lanes sorted in ascending order by key?
    fn is_sorted_key(&self) -> bool {
        let mut prev = 0u64;
        for lane in self.lanes.iter() {
            if lane.key < prev {return false} else {prev = lane.key}
        }
        return true
    }

    // Are all lanes sorted in ascending order by metadata?
    fn is_sorted_meta(&self) -> bool {
        let mut prev = 0u64;
        for lane in self.lanes.iter() {
            if lane.meta < prev {return false} else {prev = lane.meta}
        }
        return true
    }

    // Apply a series of lane-swap operations to generate a new LaneArray.
    // Each operator is a pair of input/output indices; smaller key copied
    // to the first index, larger key to the second.
    fn swap(&self, ops:&Vec<LaneSwap>) -> LaneArray {
        let mut result = self.clone();
        for LaneSwap(n1,n2) in ops.iter() {
            if self.lanes[*n1].key <= self.lanes[*n2].key {
                result.lanes[*n1] = self.lanes[*n1].clone();
                result.lanes[*n2] = self.lanes[*n2].clone();
            } else {
                result.lanes[*n1] = self.lanes[*n2].clone();
                result.lanes[*n2] = self.lanes[*n1].clone();
            }
        }
        return result
    }

    // Information-deleting analogue to swap() function, shifts up
    // by replacing any invalid inputs with a constant placeholder.
    fn shift(&self, ops:&Vec<LaneSwap>) -> LaneArray {
        let mut result = self.clone();
        for LaneSwap(n1,n2) in ops.iter() {
            if self.lanes[*n1].key < PENALTY {
                result.lanes[*n1] = self.lanes[*n1].clone();
                result.lanes[*n2] = self.lanes[*n2].clone();
            } else {
                result.lanes[*n1] = self.lanes[*n2].clone();
                result.lanes[*n2] = Lane {key:PENALTY, meta:PENALTY};
            }
        }
        return result
    }
}

impl fmt::Display for LaneArray {
    // Print the key values for all lanes.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        for lane in self.lanes.iter() {
            write!(f, "{}, ", &lane.key)?;
        }
        write!(f, ")")
    }
}

// Given a sorting function, test that it functions correctly
// and then report whether it preserves order in case of ties.
fn test_sort(len:u8, lbl:&str, sortfn:fn(&LaneArray)->LaneArray) {
    // Test that sorting is correct for each possible enable mask,
    // counting violations in both Simple and Hidden indexing modes.
    let max_mask = 1u64 << len;
    let mut err_key = 0u64;
    let mut err_meta = 0u64;
    for mask in 0..max_mask {
        let types = [LaneArrayType::Simple(mask),
                     LaneArrayType::Hidden(mask)];
        for typ in types.iter() {
            let x = LaneArray::new(len, typ);
            let y = sortfn(&x);
            if !y.is_sorted_key() {
                println!("x = {}", x);
                println!("y = {}", y);
                err_key += 1;
            }
            if !y.is_sorted_meta() {
                err_meta += 1;
            }
        }
    }

    // Summary report:
    if err_key > 0 {
        println!("{}\t Sorting error.", lbl);
    } else if err_meta > 0 {
        println!("{}\t Order not preserved.", lbl);
    } else {
        println!("{}\t All tests passed.", lbl);
    }
}

// Declare functions defining variations on the bitonic sort algorithm.
// https://en.wikipedia.org/wiki/Bitonic_sorter
fn bitonic4a(p0:&LaneArray) -> LaneArray {
    // Bitonic network, original formulation
    // https://www.inf.hs-flensburg.de/lang/algorithmen/sortieren/bitonic/bitonicen.htm
    assert_eq!(p0.lanes.len(), 4usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(3,2)]);
    let p2 = p1.swap(&vec![sw(0,2),sw(1,3)]);
    let p3 = p2.swap(&vec![sw(0,1),sw(2,3)]);
    return p3
}

fn bitonic4b(p0:&LaneArray) -> LaneArray {
    // Bitonic network, downward swaps only
    assert_eq!(p0.lanes.len(), 4usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(2,3)]);
    let p2 = p1.swap(&vec![sw(0,3),sw(1,2)]);
    let p3 = p2.swap(&vec![sw(0,1),sw(2,3)]);
    return p3
}

fn bitonic8a(p0:&LaneArray) -> LaneArray {
    // Bitonic network, original formulation
    // https://en.wikipedia.org/wiki/Bitonic_sorter#/media/File:BitonicSort1.svg
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(3,2),sw(4,5),sw(7,6)]);
    let p2 = p1.swap(&vec![sw(0,2),sw(1,3),sw(7,5),sw(6,4)]);
    let p3 = p2.swap(&vec![sw(0,1),sw(2,3),sw(5,4),sw(7,6)]);
    let p4 = p3.swap(&vec![sw(0,4),sw(1,5),sw(2,6),sw(3,7)]);
    let p5 = p4.swap(&vec![sw(0,2),sw(1,3),sw(4,6),sw(5,7)]);
    let p6 = p5.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    return p6
}

fn bitonic8b(p0:&LaneArray) -> LaneArray {
    // Bitonic network, downward swaps only
    // https://en.wikipedia.org/wiki/Bitonic_sorter#/media/File:BitonicSort.svg
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p2 = p1.swap(&vec![sw(0,3),sw(1,2),sw(4,7),sw(5,6)]);
    let p3 = p2.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p4 = p3.swap(&vec![sw(0,7),sw(1,6),sw(2,5),sw(3,4)]);
    let p5 = p4.swap(&vec![sw(0,2),sw(1,3),sw(4,6),sw(5,7)]);
    let p6 = p5.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    return p6
}

fn batcher8(p0:&LaneArray) -> LaneArray {
    // Batcher sort, aka odd-even mergesort
    // https://www.inf.hs-flensburg.de/lang/algorithmen/sortieren/networks/oemen.htm
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p2 = p1.swap(&vec![sw(0,2),sw(1,3),sw(4,6),sw(5,7)]);
    let p3 = p2.swap(&vec![sw(1,2),sw(5,6)]);
    let p4 = p3.swap(&vec![sw(0,4),sw(1,5),sw(2,6),sw(3,7)]);
    let p5 = p4.swap(&vec![sw(2,4),sw(3,5)]);
    let p6 = p5.swap(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    return p6
}

fn bubble8(p0:&LaneArray) -> LaneArray {
    // Bubble sort
    // https://www.inf.hs-flensburg.de/lang/algorithmen/sortieren/networks/sortieren.htm
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.shift(&vec![sw(0,1)]);
    let p2 = p1.shift(&vec![sw(1,2)]);
    let p3 = p2.shift(&vec![sw(0,1),sw(2,3)]);
    let p4 = p3.shift(&vec![sw(1,2),sw(3,4)]);
    let p5 = p4.shift(&vec![sw(0,1),sw(2,3),sw(4,5)]);
    let p6 = p5.shift(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p7 = p6.shift(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p8 = p7.shift(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p9 = p8.shift(&vec![sw(0,1),sw(2,3),sw(4,5)]);
    let p10 = p9.shift(&vec![sw(1,2),sw(3,4)]);
    let p11 = p10.shift(&vec![sw(0,1),sw(2,3)]);
    let p12 = p11.shift(&vec![sw(1,2)]);
    let p13 = p12.shift(&vec![sw(0,1)]);
    return p13
}

fn pairwise8(p0:&LaneArray) -> LaneArray {
    // Pairwise sorting network
    // https://en.wikipedia.org/wiki/Pairwise_sorting_network
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p2 = p1.swap(&vec![sw(0,2),sw(1,3),sw(4,6),sw(5,7)]);
    let p3 = p2.swap(&vec![sw(0,4),sw(1,5),sw(2,6),sw(3,7)]);
    let p4 = p3.swap(&vec![sw(2,4),sw(3,5)]);
    let p5 = p4.swap(&vec![sw(1,4),sw(3,6)]);
    let p6 = p5.swap(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    return p6
}

fn transpose8(p0:&LaneArray) -> LaneArray {
    // Odd-even transpose sort
    // https://www.inf.hs-flensburg.de/lang/algorithmen/sortieren/networks/oetsen.htm
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p2 = p1.swap(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p3 = p2.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p4 = p3.swap(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p5 = p4.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p6 = p5.swap(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p7 = p6.swap(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p8 = p7.swap(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    return p8
}

fn transpose8s(p0:&LaneArray) -> LaneArray {
    // Information-deleting analogue to "transpose8".
    assert_eq!(p0.lanes.len(), 8usize);
    let p1 = p0.shift(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p2 = p1.shift(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p3 = p2.shift(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p4 = p3.shift(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p5 = p4.shift(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p6 = p5.shift(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    let p7 = p6.shift(&vec![sw(0,1),sw(2,3),sw(4,5),sw(6,7)]);
    let p8 = p7.shift(&vec![sw(1,2),sw(3,4),sw(5,6)]);
    return p8
}

fn transpose3s(p0:&LaneArray) -> LaneArray {
    // Test variants of "transpose8s" with unusual sizes.
    assert_eq!(p0.lanes.len(), 3usize);
    let p1 = p0.shift(&vec![sw(0,1)]);
    let p2 = p1.shift(&vec![sw(1,2)]);
    let p3 = p2.shift(&vec![sw(0,1)]);
    return p3
}

fn transpose5s(p0:&LaneArray) -> LaneArray {
    // Test variants of "transpose8s" with unusual sizes.
    assert_eq!(p0.lanes.len(), 5usize);
    let p1 = p0.shift(&vec![sw(0,1),sw(2,3)]);
    let p2 = p1.shift(&vec![sw(1,2),sw(3,4)]);
    let p3 = p2.shift(&vec![sw(0,1),sw(2,3)]);
    let p4 = p3.shift(&vec![sw(1,2),sw(3,4)]);
    let p5 = p4.shift(&vec![sw(0,1),sw(2,3)]);
    return p5
}

fn transpose6s(p0:&LaneArray) -> LaneArray {
    // Test variants of "transpose8s" with unusual sizes.
    assert_eq!(p0.lanes.len(), 6usize);
    let p1 = p0.shift(&vec![sw(0,1),sw(2,3),sw(4,5)]);
    let p2 = p1.shift(&vec![sw(1,2),sw(3,4)]);
    let p3 = p2.shift(&vec![sw(0,1),sw(2,3),sw(4,5)]);
    let p4 = p3.shift(&vec![sw(1,2),sw(3,4)]);
    let p5 = p4.shift(&vec![sw(0,1),sw(2,3),sw(4,5)]);
    let p6 = p5.shift(&vec![sw(1,2),sw(3,4)]);
    return p6
}

// Test each of the defined sorting functions.
fn main() {
    test_sort(4, "bitonic4a",   bitonic4a);
    test_sort(4, "bitonic4b",   bitonic4b);
    test_sort(8, "bitonic8a",   bitonic8a);
    test_sort(8, "bitonic8b",   bitonic8b);
    test_sort(8, "batcher8",    batcher8);
    test_sort(8, "bubble8\t",   bubble8);
    test_sort(8, "pairwise8",   pairwise8);
    test_sort(8, "transpose8",  transpose8);
    test_sort(8, "transpose8s", transpose8s);
    test_sort(3, "transpose3s", transpose3s);
    test_sort(5, "transpose5s", transpose5s);
    test_sort(6, "transpose6s", transpose6s);
}
