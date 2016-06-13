use std::f32;
use std::mem;
use super::{Hit, Ray, Tri, intersect};
use bb::Aabb;
use watertight_triangle::max_dim;
use conv::prelude::*;

pub enum Node {
    Internal(Aabb, Aabb, Box<(Node, Node)>),
    Leaf(u32, u32),
}

const MAX_LEAF_SIZE: usize = 10;
const MAX_DEPTH: u32 = 100;

pub fn construct(tris: &mut [Tri], bb: Aabb) -> Node {
    split(tris, bb, 0, 0)
}

fn split(tris: &mut [Tri], bb: Aabb, tri_offset: usize, depth: u32) -> Node {
    let axis = max_dim(bb.max - bb.min);
    let pivot = bb.min[axis] * 0.5 + bb.max[axis] * 0.5;
    let reached_epsilon = bb.min[axis] == pivot || bb.max[axis] == pivot;
    if tris.len() <= MAX_LEAF_SIZE || depth > MAX_DEPTH || reached_epsilon {
        if tris.len() > MAX_LEAF_SIZE {
            println!("Giving up on {} tris", tris.len());
        }
        let start = tri_offset.value_as::<u32>().unwrap();
        let end = start + tris.len().value_as::<u32>().unwrap();
        return Node::Leaf(start, end);
    }
    let left_count = partition(tris, pivot, axis);
    if left_count == 0 {
        // Everything was on the right, so instead build a node for that half
        return split(tris, bb.with_min(axis, pivot), tri_offset, depth);
    }
    if left_count == tris.len() {
        // Everything was on the right, so instead build a node for that half
        return split(tris, bb.with_max(axis, pivot), tri_offset, depth);
    }
    let (left, right) = tris.split_at_mut(left_count);
    let left_bb = Aabb::for_tris(left);
    let right_bb = Aabb::for_tris(right);
    // Deliberately not passing left_bb and right_bb to each child.
    // Instead we use the BIH "global heuristic" which uses a regular grid in the scene BB
    let left_child = split(left, bb.with_max(axis, pivot), tri_offset, depth + 1);
    let right_child = split(right,
                            bb.with_min(axis, pivot),
                            tri_offset + left_count,
                            depth + 1);
    Node::Internal(left_bb, right_bb, Box::new((left_child, right_child)))
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

pub fn traverse<'a>(tris: &'a [Tri], node: &Node, r: &Ray, t0: f32, mut t1: f32) -> Option<Hit<'a>> {
    match *node {
        Node::Leaf(start, end) => {
            let leaf_tris = &tris[start as usize..end as usize];
            intersect(leaf_tris, r)
        }
        Node::Internal(ref bb1, ref bb2, ref children) => {
            let (mut hit1, mut hit2) = (None, None);
            if let Some((t0, t1)) = bb1.intersect(r, t0, t1) {
                hit1 = traverse(tris, &children.0, r, t0, t1);
            }
            if let Some(ref hit) = hit1 {
                // Don't go further than necessary
                t1 = hit.t;
            }
            if let Some((t0, t1)) = bb2.intersect(r, t0, t1) {
                hit2 = traverse(tris, &children.1, r, t0, t1);
            }
            match (hit1, hit2) {
                (Some(hit1), Some(hit2)) => {
                    if hit1.t < hit2.t {
                        Some(hit1)
                    } else {
                        Some(hit2)
                    }
                }
                (None, hit) => hit,
                (hit, None) => hit,
            }
        }
    }
}
