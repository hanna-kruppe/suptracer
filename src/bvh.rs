use std::f32;
use std::mem;
use std::u32;
use std::sync::atomic::{AtomicUsize, Ordering};
use cgmath::Vector3;
use conv::prelude::*;
use rayon;
use watertight_triangle::{self, max_dim};
use arrayvec::ArrayVec;

use super::timeit;
use geom::{Hit, Ray, Tri, TriSliceExt};
use bb::Aabb;

pub struct Bvh {
    nodes: Box<[CompactNode]>,
}

/// A temporary node built during construction. Converted to CompactNode afterwards.
enum BuildNode {
    Leaf(Aabb, u32, u32),
    Interior(Aabb, Box<(BuildNode, BuildNode)>, u8),
}

const LEAF_OR_NODE_MASK: u32 = 1 << 31;

struct CompactNode {
    bb: Aabb,
    /// In leaf nodes, the (absolute) offset of the primitives.
    /// In interior nodes, the (absolute) offset of the second child.
    offset: u32,
    /// The MSB of this field indicates whether it's a leaf (0) or an interior node (1).
    /// In leaf nodes, it also contains the number of triangles (< 2^31).
    /// In interior nodes, the lower bits store the axis.
    payload: u32,
}

/// Unpacked representation of a node.
/// Only used as a temporary, not stored in BVH.
/// The AABB is omitted since its representation is the same for leaves and interior nodes.
enum UnpackedNode {
    Leaf {
        tri_start: u32,
        tri_end: u32,
    },
    Interior {
        second_child: NodeId,
        axis: u32,
    },
}

impl CompactNode {
    fn unpack(&self) -> UnpackedNode {
        if self.payload & LEAF_OR_NODE_MASK == 0 {
            UnpackedNode::Leaf {
                tri_start: self.offset,
                tri_end: self.offset + self.payload,
            }
        } else {
            UnpackedNode::Interior {
                second_child: NodeId(self.offset),
                axis: self.payload & !LEAF_OR_NODE_MASK,
            }
        }
    }
}

#[derive(Copy,Clone,Debug,PartialEq,Eq)]
struct NodeId(u32);

impl NodeId {
    fn to_index(&self) -> usize {
        debug_assert!(self.0.value_as::<usize>().is_ok());
        self.0 as usize
    }

    fn left_child(&self) -> Self {
        NodeId(self.0 + 1)
    }
}

impl Bvh {
    fn compactify(root: BuildNode, node_count: usize) -> Bvh {
        let mut nodes = Vec::with_capacity(node_count);
        compactify(&mut nodes, root);
        assert_eq!(nodes.len(),
                   node_count,
                   "Builder reported wrong number of nodes");
        Bvh { nodes: nodes.into_boxed_slice() }
    }
}

fn compactify(nodes: &mut Vec<CompactNode>, node: BuildNode) -> NodeId {
    let id = NodeId(nodes.len().value_as().unwrap());
    const INVALID_ID: u32 = u32::MAX;
    match node {
        BuildNode::Leaf(bb, start, end) => {
            nodes.push(CompactNode {
                bb: bb,
                offset: start,
                payload: end - start,
            });
        }
        BuildNode::Interior(bb, children, axis) => {
            assert!(axis < 3);
            nodes.push(CompactNode {
                bb: bb,
                offset: INVALID_ID,
                payload: LEAF_OR_NODE_MASK | axis.value_as::<u32>().unwrap(),
            });
            let children = *children;  // Workaround for missing box pattern
            let id_l = compactify(nodes, children.0);
            let id_r = compactify(nodes, children.1);
            assert_eq!(id_l.0, id.0 + 1);
            nodes[id.to_index()].offset = id_r.0;
        }
    }
    id
}

/// All data needed to build a subtree of a BVH
struct SubtreeBuilder<'c, 't> {
    tris: &'t mut [Tri],
    bb: Aabb,
    tri_offset: u32,
    node_count: &'c AtomicUsize,
    depth: usize,
}

struct SahData {
    axis: usize,
    parent_surface_area: f32,
    centroids: Vec<Vector3<f32>>,
    centroid_bb: Aabb,
    buckets: [Bucket; BUCKET_COUNT],
}

impl SahData {
    fn bucket_borders(&self) -> (f32, f32) {
        (self.centroid_bb.min[self.axis], self.centroid_bb.max[self.axis])
    }

    fn bucket(&self, x: &Vector3<f32>) -> usize {
        let (left_border, right_border) = self.bucket_borders();
        let relative_pos = (x[self.axis] - left_border) / (right_border - left_border);
        let b = (BUCKET_COUNT as f32 * relative_pos) as usize;
        if b == BUCKET_COUNT {
            b - 1
        } else {
            b
        }
    }

    fn best_split(&self) -> (f32, usize) {
        const TRAVERSAL_COST: f32 = 8.0;
        let mut costs = [f32::NAN; BUCKET_COUNT - 1];
        for (i, cost) in costs.iter_mut().enumerate() {
            let (mut b0, mut b1) = (Aabb::empty(), Aabb::empty());
            let (mut count0, mut count1) = (0, 0);
            let split_idx = i + 1;
            for bucket in &self.buckets[..split_idx] {
                b0 = b0.union(&bucket.bb);
                count0 += bucket.count;
            }
            for bucket in &self.buckets[split_idx..] {
                b1 = b1.union(&bucket.bb);
                count1 += bucket.count;
            }
            *cost = TRAVERSAL_COST +
                    (count0 as f32 * b0.surface_area() + count1 as f32 * b1.surface_area()) /
                    self.parent_surface_area;
        }

        let mut min_cost = costs[0];
        let mut min_cost_idx = 0;
        for (i, &cost) in costs.iter().enumerate() {
            if cost < min_cost {
                min_cost = cost;
                min_cost_idx = i;
            }
        }
        (min_cost, min_cost_idx)
    }
}

#[derive(Copy, Clone, Debug)]
struct Bucket {
    count: u32,
    bb: Aabb,
}

impl Bucket {
    fn empty() -> Self {
        Bucket {
            count: 0,
            bb: Aabb::empty(),
        }
    }
}

const BUCKET_COUNT: usize = 16;
const MAX_DEPTH: usize = 64;

impl<'c, 't> SubtreeBuilder<'c, 't> {
    fn new(tris: &'t mut [Tri],
           bb: Aabb,
           tri_offset: u32,
           node_count: &'c AtomicUsize,
           depth: usize)
           -> Self {
        assert!(depth < MAX_DEPTH,
                "BVH is becoming unreasonably deep --- infinite loop?");
        node_count.fetch_add(1, Ordering::SeqCst);
        SubtreeBuilder {
            tris: tris,
            bb: bb,
            tri_offset: tri_offset,
            node_count: node_count,
            depth: depth + 1,
        }
    }

    fn split(mut self, sah: &SahData, split_bucket: usize) -> (Self, Self) {
        let mid = self.partition(sah, split_bucket);
        let (l, r) = self.tris.split_at_mut(mid);
        // FIXME compute BBs as union of bucket groups
        let (bb_l, bb_r) = (Aabb::new(l), Aabb::new(r));
        let offset_l = self.tri_offset;
        let offset_r = offset_l + mid.value_as::<u32>().unwrap();
        (SubtreeBuilder::new(l, bb_l, offset_l, self.node_count, self.depth),
         SubtreeBuilder::new(r, bb_r, offset_r, self.node_count, self.depth))
    }

    fn make_leaf(self) -> BuildNode {
        let tri_end = self.tri_offset + self.tris.len().value_as::<u32>().unwrap();
        BuildNode::Leaf(self.bb, self.tri_offset, tri_end)
    }

    fn build(self) -> BuildNode {
        if self.tris.len() == 1 {
            return self.make_leaf();
        }
        let sah = if let Ok(s) = self.sah_data() {
            s
        } else {
            // Centroids are all clumped together, give up
            return self.make_leaf();
        };
        let (split_cost, split_bucket) = sah.best_split();
        let leaf_cost = self.tris.len() as f32;
        if leaf_cost <= split_cost {
            self.make_leaf()
        } else {
            let bb = self.bb;
            let (l, r) = self.split(&sah, split_bucket);
            let children = rayon::join(move || l.build(), move || r.build());
            BuildNode::Interior(bb, Box::new(children), sah.axis.value_as::<u8>().unwrap())
        }
    }

    fn sah_data(&self) -> Result<SahData, ()> {
        let centroids: Vec<_> = self.tris.iter().map(Tri::centroid).collect();
        let centroid_bb = Aabb::from_points(&centroids);
        if centroid_bb.min == centroid_bb.max {
            return Err(());
        }
        let axis = max_dim(centroid_bb.max - centroid_bb.min);
        let mut sah = SahData {
            axis: axis,
            parent_surface_area: self.bb.surface_area(),
            centroids: centroids,
            centroid_bb: centroid_bb,
            buckets: [Bucket::empty(); BUCKET_COUNT],
        };
        for c in &sah.centroids {
            let b = sah.bucket(c);
            sah.buckets[b].count += 1;
            sah.buckets[b].bb.add_point(c);
        }
        Ok(sah)
    }

    fn partition(&mut self, sah: &SahData, split_bucket: usize) -> usize {
        // The tris slice is composed of three sub-slices (in this order):
        // 1. Those known to be left of the split plane,
        // 2. The still-unclassified ones
        // 3. Those known to be right of the split plane
        // We start with all tris uncategorized and grow the left and right slices in the loop.
        // The slices are represented by integers (left, remaining) s.t. tris[0..left] is the left
        // slice, tris[left..left+remaining] is the uncategorized slice, and tris[left+remaining..]
        // is the right slice.
        let mut left = 0;
        let mut remaining = self.tris.len();
        let is_left = |tri: &Tri| sah.bucket(&tri.centroid()) <= split_bucket;
        // TODO rewrite to use sah.centroids and bucket projection
        while remaining > 0 {
            let (uncategorized, _right) = self.tris[left..].split_at_mut(remaining);
            // Split off the first element of uncategorized, to be able to swap it if necessary
            let (uncat_start, uncat_rest) = uncategorized.split_at_mut(1);
            let tri = &mut uncat_start[0];
            remaining -= 1;
            if is_left(tri) {
                left += 1;
            } else {
                if let Some(last_uncat) = uncat_rest.last_mut() {
                    mem::swap(tri, last_uncat);
                }
            }
        }
        left
    }
}

pub fn construct(tris: &mut [Tri], bb: Aabb) -> Bvh {
    let msg = format!("built BVH for {} tris", tris.len());
    let node_count = AtomicUsize::new(0);
    let (bvh, _) = timeit(&msg, move || {
        let builder = SubtreeBuilder::new(tris, bb, 0, &node_count, 0);
        let root = builder.build();
        Bvh::compactify(root, node_count.load(Ordering::SeqCst))
    });
    bvh
}

pub fn traverse<'a>(tris: &[Tri], tree: &Bvh, r: &Ray) -> Hit {
    let sign = [(r.d[0] < 0.0) as usize, (r.d[1] < 0.0) as usize, (r.d[2] < 0.0) as usize];
    let r_data = watertight_triangle::RayData::new(r.o, r.d);
    let inv_dir = 1.0 / r.d;
    let mut hit = Hit::none();

    // FIXME this should be a SmallVec or ArrayVec
    let mut todo = ArrayVec::<[_; MAX_DEPTH]>::new();
    todo.push(NodeId(0));
    while let Some(id) = todo.pop() {
        let node = &tree.nodes[id.to_index()];
        if !node.bb.intersect(r, sign, inv_dir) {
            continue;
        }
        match node.unpack() {
            UnpackedNode::Leaf { tri_start, tri_end } => {
                let start = tri_start.value_as::<usize>().unwrap();
                let end = tri_end.value_as::<usize>().unwrap();
                tris[start..end].intersect(tri_start, r, &r_data, &mut hit);
            }
            UnpackedNode::Interior { second_child, axis } => {
                // TODO ordered traversal perhaps?
                // On the Stanford Bunny at least, this is faster than any
                // kind of _axis-based traversal I've tried =(
                let dir_negative = sign[axis as usize] == 1;
                if dir_negative {
                    todo.push(id.left_child());
                    todo.push(second_child);
                } else {
                    todo.push(second_child);
                    todo.push(id.left_child());
                }
            }
        }
    }
    hit
}
