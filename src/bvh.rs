use super::{Config, timeit};
use arrayvec::ArrayVec;
use beebox::{self, Aabb};
use beevage::{self, Axis};
use cast::{u32, usize};
use geom::{Hit, Ray, Tri, TriSliceExt};
use rayon::prelude::*;
use std::u32;
use watertri;

pub struct Bvh {
    nodes: Box<[CompactNode]>,
}

const LEAF_OR_NODE_MASK: u32 = 1 << 31;

struct CompactNode {
    bb: Aabb,
    /// In leaf nodes, the (absolute) offset of the primitives.
    /// In interior nodes, the (absolute) offset of the second child.
    offset: u32,
    /// The MSB of this field indicates whether it's a leaf (0) or an interior node (1).
    /// In leaf nodes, it also contains the number of triangles (< 2^31).
    /// In interior nodes, the lower bits of this field store the axis.
    payload: u32,
}

/// Unpacked representation of a node.
/// Only used as a temporary, not stored in BVH.
/// The AABB is omitted since its representation is the same for leaves and interior nodes.
enum UnpackedNode {
    Leaf { start: u32, end: u32 },
    Interior { second_child: NodeId, axis: u8 },
}

impl CompactNode {
    fn unpack(&self) -> UnpackedNode {
        if self.payload & LEAF_OR_NODE_MASK == 0 {
            UnpackedNode::Leaf {
                start: self.offset,
                end: self.offset + self.payload,
            }
        } else {
            UnpackedNode::Interior {
                second_child: NodeId(self.offset),
                axis: self.payload as u8,
            }
        }
    }
}

#[derive(Copy,Clone,Debug,PartialEq,Eq)]
struct NodeId(u32);

impl NodeId {
    fn to_index(&self) -> usize {
        usize(self.0)
    }

    fn left_child(&self) -> Self {
        NodeId(self.0 + 1)
    }
}

impl Bvh {
    fn compactify(root: beevage::Node, node_count: usize) -> Bvh {
        let mut nodes = Vec::with_capacity(node_count);
        compactify(&mut nodes, root);
        assert_eq!(nodes.len(),
                   node_count,
                   "Builder reported wrong number of nodes");
        Bvh { nodes: nodes.into_boxed_slice() }
    }
}

fn compactify(nodes: &mut Vec<CompactNode>, node: beevage::Node) -> NodeId {
    let id = NodeId(u32(nodes.len()).unwrap());
    const INVALID_ID: u32 = u32::MAX;
    match node {
        beevage::Node::Leaf { bb, primitive_range } => {
            let payload = u32(primitive_range.len()).unwrap();
            assert!(payload & LEAF_OR_NODE_MASK == 0);
            nodes.push(CompactNode {
                bb: bb,
                offset: u32(primitive_range.start).unwrap(),
                payload: payload,
            });
        }
        beevage::Node::Inner { bb, children, axis } => {
            let axis_id = match axis {
                Axis::X => 0,
                Axis::Y => 1,
                Axis::Z => 2,
            };
            nodes.push(CompactNode {
                bb: bb,
                offset: INVALID_ID,
                payload: LEAF_OR_NODE_MASK | axis_id,
            });
            let children = *children; // Workaround for missing box pattern
            let id_l = compactify(nodes, children.0);
            let id_r = compactify(nodes, children.1);
            assert_eq!(id_l.0, id.0 + 1);
            nodes[id.to_index()].offset = id_r.0;
        }
    }
    id
}

const MAX_DEPTH: usize = 64;

pub fn construct(tris: &[Tri], cfg: &Config) -> (Bvh, Vec<Tri>) {
    let msg = format!("building BVH for {} tris", tris.len());
    let (res, _) = timeit(&msg, move || {
        let bb = tris.bbox();
        let config = beevage::Config {
            bucket_count: usize(cfg.sah_buckets),
            traversal_cost: cfg.sah_traversal_cost,
            max_depth: MAX_DEPTH,
        };
        let beevage::Bvh { root, node_count, primitives } = beevage::binned_sah(config, tris, bb);
        let mut bvh_tris = Vec::with_capacity(tris.len());
        primitives.into_par_iter().map(|p| tris[p.index()].clone()).collect_into(&mut bvh_tris);
        (Bvh::compactify(root, node_count), bvh_tris)
    });
    res
}


pub fn traverse(tris: &[Tri], tree: &Bvh, r: &Ray) -> Hit {
    // TODO make layout breadth-first and use distance-based traversal
    //      (isect both children, go to nearer one)
    // TODO then try this:
    // > Stackless Multi-BVH Traversal for CPU, MIC and GPU Ray Tracing
    // > Attila T. Áfra and László Szirmay-Kalos
    // > Computer Graphics Forum (2013)
    let r_tri = watertri::RayData::new(r.o, r.d);
    let r_box = beebox::RayData::new(r.o, r.d);
    let mut hit = Hit::none();

    let mut todo = ArrayVec::<[_; MAX_DEPTH]>::new();
    todo.push(NodeId(0));
    while let Some(id) = todo.pop() {
        r.traversal_steps.set(r.traversal_steps.get() + 1);
        let node = &tree.nodes[id.to_index()];
        if !node.bb.intersects(&r_box, 0.0, r.t_max.get()) {
            continue;
        }
        match node.unpack() {
            UnpackedNode::Leaf { start, end } => {
                tris[usize(start)..usize(end)].intersect(start, r, &r_tri, &mut hit);
            }
            UnpackedNode::Interior { second_child, axis } => {
                if r.d[usize(axis)] < 0.0 {
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
