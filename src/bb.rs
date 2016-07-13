use std::fmt;
use std::f32;
use cgmath::{Vector3, vec3};

use geom::{Ray, Tri};

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Aabb {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl Aabb {
    pub fn new(tris: &[Tri]) -> Self {
        let mut res = Aabb::empty();
        for tri in tris {
            res.add_point(&tri.a);
            res.add_point(&tri.b);
            res.add_point(&tri.c);
        }
        res
    }

    pub fn from_points(points: &[Vector3<f32>]) -> Self {
        let mut res = Aabb::empty();
        for p in points {
            res.add_point(p);
        }
        res
    }

    pub fn empty() -> Self {
        let min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let max = -min;
        Aabb {
            min: min,
            max: max,
        }
    }

    pub fn add_point(&mut self, v: &Vector3<f32>) {
        // FIXME f32::min calls fmin, which is robust against NaN but may be
        // unnecessarily slow since it can't be mapped to SSE
        self.min.x = self.min.x.min(v.x);
        self.min.y = self.min.y.min(v.y);
        self.min.z = self.min.z.min(v.z);
        self.max.x = self.max.x.max(v.x);
        self.max.y = self.max.y.max(v.y);
        self.max.z = self.max.z.max(v.z);
    }

    pub fn union(&self, other: &Self) -> Self {
        Aabb {
            min: vec3(self.min.x.min(other.min.x),
                      self.min.y.min(other.min.y),
                      self.min.z.min(other.min.z)),
            max: vec3(self.max.x.max(other.max.x),
                      self.max.y.max(other.max.y),
                      self.max.z.max(other.max.z)),
        }
    }

    pub fn surface_area(&self) -> f32 {
        if self == &Aabb::empty() {
            return 0.0;
        }
        let d = self.max - self.min;
        let area = 2.0 * (d.x * d.y + d.x * d.z + d.y * d.z);
        if !area.is_finite() {
            println!("inf surface area: {:?}", self);
        }
        area
    }

    // Williams, Amy, et al. "An efficient and robust ray-box intersection algorithm."
    // ACM SIGGRAPH 2005 Courses. ACM, 2005.
    pub fn intersect(&self, r: &Ray, sign: [usize; 3], inv_dir: Vector3<f32>) -> bool {
        let p = [self.min, self.max];
        let mut tmin = (p[sign[0]].x - r.o.x) * inv_dir.x;
        let mut tmax = (p[1 - sign[0]].x - r.o.x) * inv_dir.x;
        let tymin = (p[sign[1]].y - r.o.y) * inv_dir.y;
        let tymax = (p[1 - sign[1]].y - r.o.y) * inv_dir.y;
        if tmin > tymax || tymin > tmax {
            return false;
        }
        tmin = tmin.min(tymin);
        tmax = tmax.max(tymax);
        let tzmin = (p[sign[2]].z - r.o.z) * inv_dir.z;
        let tzmax = (p[1 - sign[2]].z - r.o.z) * inv_dir.z;
        if tmin > tzmax || tzmin > tmax {
            return false;
        }
        tmin = tmin.min(tzmin);
        tmax = tmax.max(tzmax);
        tmin < r.t_max.get() && tmax > 0.0
    }
}

impl fmt::Display for Aabb {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "[{}, {}, {}]..[{}, {}, {}]",
               self.min.x,
               self.min.y,
               self.min.z,
               self.max.x,
               self.max.y,
               self.max.z)
    }
}
