

use beebox::Aabb;
use cast::u32;
use cgmath::Vector3;
use film::Color;
use std::{f32, u32};
use std::cell::Cell;
use watertri;

#[derive(Clone, Debug)]
pub struct Tri {
    pub a: Vector3<f32>,
    pub b: Vector3<f32>,
    pub c: Vector3<f32>,
    pub color: Color,
}

impl Tri {
    pub fn bbox(&self) -> Aabb {
        Aabb::new([self.a, self.b, self.c].iter().cloned())
    }
}

#[derive(Debug)]
pub struct Ray {
    pub o: Vector3<f32>,
    pub d: Vector3<f32>,
    pub t_max: Cell<f32>,
    pub traversal_steps: Cell<u32>,
}

impl Ray {
    pub fn new(origin: Vector3<f32>, direction: Vector3<f32>) -> Ray {
        Ray {
            o: origin,
            d: direction,
            t_max: Cell::new(f32::INFINITY),
            traversal_steps: Cell::new(0),
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

    pub fn replace(&mut self, tri_id: u32, i: watertri::Intersection) {
        self.tri_id = tri_id;
        self.t = i.t;
        self.u = i.u;
        self.v = i.v;
        self.w = i.w;
    }
}

pub trait TriSliceExt {
    fn bbox(&self) -> Aabb;
    fn intersect(&self, offset: u32, ray: &Ray, ray_data: &watertri::RayData, hit: &mut Hit);
}

impl TriSliceExt for [Tri] {
    fn intersect(&self, offset: u32, ray: &Ray, ray_data: &watertri::RayData, hit: &mut Hit) {
        for (i, tri) in self.iter().enumerate() {
            if let Some(intersection) = ray_data.intersect(tri.a, tri.b, tri.c) {
                if intersection.t < ray.t_max.get() {
                    ray.t_max.set(intersection.t);
                    hit.replace(offset + u32(i).unwrap(), intersection);
                }
            }
        }
    }

    fn bbox(&self) -> Aabb {
        let mut res = Aabb::empty();
        for tri in self {
            res = res.union(tri.bbox());
        }
        res
    }
}
