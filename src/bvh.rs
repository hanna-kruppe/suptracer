use std::f32;
use std::mem;
use std::u32;
// use rayon;
use super::{Hit, Ray, Tri, intersect, timeit};
use bb::Aabb;
use watertight_triangle::max_dim;
use conv::prelude::*;

pub struct Bvh {
    nodes: Box<[CompactNode]>,
}

struct CompactNode {
    bb: Aabb,
    /// In leaf nodes, the (absolute) offset of the primitives.
    /// In interior nodes, the (absolute) offset of the second child.
    offset: u32,
    /// In leaf nodes, the number of triangles (highest bit unset).
    /// In interior nodes, the MSB is set and the 2 LSBs indicate the axis.
    payload: u32,
}

/// Unpacked representation of a node.
/// Only used as a temporary, not stored in BVH.
enum UnpackedNode<'a> {
    Leaf {
        bb: &'a Aabb,
        tri_start: u32,
        tri_end: u32,
    },
    Interior {
        bb: &'a Aabb,
        second_child: NodeId,
        _axis: usize,
    },
}

impl CompactNode {
    fn unpack(&self) -> UnpackedNode {
        if self.payload & LEAF_OR_NODE_MASK == 0 {
            UnpackedNode::Leaf {
                bb: &self.bb,
                tri_start: self.offset,
                tri_end: self.offset + self.payload,
            }
        } else {
            UnpackedNode::Interior {
                bb: &self.bb,
                second_child: NodeId(self.offset),
                _axis: (self.payload & LEAF_OR_NODE_MASK).value_as::<usize>().unwrap(),
            }
        }
    }
}

#[derive(Debug,PartialEq,Eq)]
struct NodeId(u32);

impl NodeId {
    fn to_index(&self) -> usize {
        self.0.value_as::<usize>().unwrap()
    }

    fn left_child(&self) -> Self {
        NodeId(self.0 + 1)
    }
}

struct Builder<'a> {
    nodes: Vec<CompactNode>,
    tri_offset: u32,
    tris: &'a mut [Tri],
}

const INVALID_ID: NodeId = NodeId(u32::MAX);
const LEAF_OR_NODE_MASK: u32 = 1 << 31;

impl<'a> Builder<'a> {
    fn new(tris: &'a mut [Tri]) -> Self {
        Builder {
            nodes: Vec::with_capacity(tris.len()),
            tri_offset: 0,
            tris: tris,
        }
    }

    /// Create a leaf node.
    fn leaf(&mut self, bb: Aabb, count: u32) -> NodeId {
        assert!(count & LEAF_OR_NODE_MASK == 0,
                "leaf's primitive count has MSB set");
        let id = NodeId(self.nodes.len().value_as().unwrap());
        assert!(id != INVALID_ID);
        self.nodes.push(CompactNode {
            bb: bb,
            offset: self.tri_offset,
            payload: count,
        });
        self.tri_offset += count;
        id
    }

    /// Create an interior node without children. This is an invalid state
    /// and the children must be added later during construction.
    fn start_interior(&mut self, bb: Aabb, axis: usize) -> NodeId {
        let axis = axis.value_as::<u32>().unwrap();
        assert!(axis & LEAF_OR_NODE_MASK == 0);
        let id = NodeId(self.nodes.len().value_as().unwrap());
        assert!(id != INVALID_ID);
        self.nodes.push(CompactNode {
            bb: bb,
            offset: INVALID_ID.0,
            payload: axis | LEAF_OR_NODE_MASK,
        });
        id
    }

    /// Fill in the child offset in an unfinished interior node.
    fn finish_interior(&mut self,
                       parent: NodeId,
                       left_child: NodeId,
                       right_child: NodeId)
                       -> NodeId {
        assert!(parent.0 + 1 == left_child.0,
                "nodes not in depth-first order");
        self.nodes[parent.to_index()].offset = right_child.0;
        parent
    }

    fn partition(&mut self, tri_count: u32, pivot: f32, axis: usize) -> (u32, Aabb, Aabb) {
        let start = self.tri_offset.value_as::<usize>().unwrap();
        let end = start + tri_count.value_as::<usize>().unwrap();
        let tris = &mut self.tris[start..end];
        let left_count = partition(tris, pivot, axis);
        let bb_l = Aabb::new(&tris[..left_count]);
        let bb_r = Aabb::new(&tris[left_count..]);
        (left_count.value_as::<u32>().unwrap(), bb_l, bb_r)
    }

    fn finish(self) -> Bvh {
        Bvh { nodes: self.nodes.into_boxed_slice() }
    }
}

const MAX_LEAF_SIZE: u32 = 8;
const MAX_DEPTH: u32 = 100;

pub fn construct(tris: &mut [Tri], bb: Aabb) -> Bvh {
    let tri_count = tris.len().value_as::<u32>().unwrap();
    let mut builder = Builder::new(tris);
    let (bvh, _) = timeit(&format!("built BVH for {} tris", tri_count), move || {
        let root_id = split(&mut builder, tri_count, bb, 0);
        assert!(root_id == NodeId(0));
        builder.finish()
    });
    bvh
}

fn split(builder: &mut Builder, tri_count: u32, bb: Aabb, depth: u32) -> NodeId {
    // FIXME this split plane (middle of longest axis) is said to perform relatively badly
    // TODO implement SBVH
    assert!(depth < MAX_DEPTH, "BVH is pretty deep (infinite loop?)");
    if tri_count <= MAX_LEAF_SIZE {
        return builder.leaf(bb, tri_count);
    }
    let (axis, left_count, bb_l, bb_r) = find_good_split(builder, tri_count, bb.clone());
    let this_node = builder.start_interior(bb, axis);
    let left_child = split(builder, left_count, bb_l, depth + 1);
    let right_child = split(builder, tri_count - left_count, bb_r, depth + 1);
    builder.finish_interior(this_node, left_child, right_child)
}

fn find_good_split(builder: &mut Builder, tri_count: u32, bb: Aabb) -> (usize, u32, Aabb, Aabb) {
    let axis = max_dim(bb.max - bb.min);
    let pivot = bb.min[axis] * 0.5 + bb.max[axis] * 0.5;
    assert!(bb.min[axis] < pivot && pivot < bb.max[axis],
            "reached infinitesimal region");
    let (left_count, bb_l, bb_r) = builder.partition(tri_count, pivot, axis);
    // If the pivot put all primitives into one child, move the pivot and try again.
    if left_count == 0 {
        find_good_split(builder, tri_count, bb.with_max(axis, pivot))
    } else if left_count == tri_count {
        find_good_split(builder, tri_count, bb.with_min(axis, pivot))
    } else {
        (axis, left_count, bb_l, bb_r)
    }
}

fn partition(tris: &mut [Tri], pivot: f32, axis: usize) -> usize {
    // The tris slice is composed of three sub-slices (in this order):
    // 1. Those known to be left of the split plane,
    // 2. The still-unclassified ones
    // 3. Those known to be right of the split plane
    // We start with all tris uncategorized and grow the left and right slices in the loop.
    // The slices are represented by integers (left, remaining) s.t. tris[0..left] is the left
    // slice, tris[left..left+remaining] is the uncategorized slice, and tris[left+remaining..]
    // is the right slice.
    let mut left = 0;
    let mut remaining = tris.len();
    while remaining > 0 {
        let (uncategorized, _right) = tris[left..].split_at_mut(remaining);
        // Split off the first element of uncategorized, to be able to swap it if necessary
        let (uncat_start, uncat_rest) = uncategorized.split_at_mut(1);
        let tri = &mut uncat_start[0];
        let centroid = (tri.a[axis] + tri.b[axis] + tri.c[axis]) / 3.0;
        remaining -= 1;
        if centroid <= pivot {
            left += 1;
        } else {
            if let Some(last_uncat) = uncat_rest.last_mut() {
                mem::swap(tri, last_uncat);
            }
        }
    }
    left
}

pub fn traverse<'a>(tris: &'a [Tri], tree: &Bvh, r: &Ray, mut tmax: f32) -> Option<Hit<'a>> {
    let mut todo = Vec::with_capacity(64);
    todo.push(NodeId(0));
    let mut closest_hit: Option<Hit> = None;
    while let Some(id) = todo.pop() {
        match tree.nodes[id.to_index()].unpack() {
            UnpackedNode::Leaf { bb, tri_start, tri_end } => {
                if bb.intersect(r, tmax).is_none() {
                    continue;
                }
                let start = tri_start.value_as::<usize>().unwrap();
                let end = tri_end.value_as::<usize>().unwrap();
                if let Some(hit) = intersect(&tris[start..end], r) {
                    if closest_hit.is_none() || tmax > hit.t {
                        tmax = hit.t;
                        closest_hit = Some(hit);
                    }
                }
            }
            UnpackedNode::Interior { bb, second_child, _axis } => {
                if bb.intersect(r, tmax).is_none() {
                    continue;
                }
                todo.push(id.left_child());
                todo.push(second_child);
                // TODO use axis
            }
        }
    }
    closest_hit
}
