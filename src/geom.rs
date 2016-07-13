use std::cell::Cell;
use std::{u32, f32};
use cgmath::Vector3;
use conv::prelude::*;
use watertight_triangle;

use super::Color;

#[derive(Clone, Debug)]
pub struct Tri {
    pub a: Vector3<f32>,
    pub b: Vector3<f32>,
    pub c: Vector3<f32>,
    pub color: Color,
}

impl Tri {
    pub fn centroid(&self) -> Vector3<f32> {
        (self.a + self.b + self.c) / 3.0
    }
}

#[derive(Debug)]
pub struct Ray {
    pub o: Vector3<f32>,
    pub d: Vector3<f32>,
    pub t_max: Cell<f32>,
}

impl Ray {
    pub fn new(origin: Vector3<f32>, direction: Vector3<f32>) -> Ray {
        Ray {
            o: origin,
            d: direction,
            t_max: Cell::new(f32::INFINITY),
        }
    }
}

const INVALID_ID: u32 = u32::MAX;

pub struct Hit {
    pub tri_id: u32,
    pub t: f32,
    pub u: f32,
    pub v: f32,
    pub w: f32,
}

impl Hit {
    pub fn none() -> Hit {
        Hit {
            tri_id: INVALID_ID,
            t: f32::NAN,
            u: f32::NAN,
            v: f32::NAN,
            w: f32::NAN,
        }
    }

    pub fn is_valid(&self) -> bool {
        if self.tri_id == INVALID_ID {
            debug_assert!(self.u.is_nan());
            debug_assert!(self.v.is_nan());
            debug_assert!(self.w.is_nan());
            false
        } else {
            debug_assert!(!self.u.is_nan());
            debug_assert!(!self.v.is_nan());
            debug_assert!(!self.w.is_nan());
            true
        }
    }

    pub fn replace(&mut self, tri_id: u32, i: watertight_triangle::Intersection) {
        self.tri_id = tri_id;
        self.t = i.t;
        self.u = i.u;
        self.v = i.v;
        self.w = i.w;
    }
}

pub trait TriSliceExt {
    fn intersect(&self,
                 offset: u32,
                 ray: &Ray,
                 ray_data: &watertight_triangle::RayData,
                 hit: &mut Hit);
}

impl TriSliceExt for [Tri] {
    fn intersect(&self,
                 offset: u32,
                 ray: &Ray,
                 ray_data: &watertight_triangle::RayData,
                 hit: &mut Hit) {
        for (i, tri) in self.iter().enumerate() {
            let corners = (tri.a, tri.b, tri.c);
            if let Some(intersection) = watertight_triangle::intersect(corners, ray_data) {
                if intersection.t < ray.t_max.get() {
                    ray.t_max.set(intersection.t);
                    hit.replace(offset + i.value_as::<u32>().unwrap(), intersection);
                }
            }
        }
    }
}
