use super::{Ray, Tri};
use cgmath::{Vector3, vec3};
use std::fmt;
use std::f32;

#[derive(Copy, Clone, Debug)]
pub struct Aabb {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl Aabb {
    pub fn for_tris(tris: &[Tri]) -> Aabb {
        let mut min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = -min;
        // FIXME f32::min calls fmin, which is robust against NaN but relatively slow,
        // especially since it can't be vectorized
        for tri in tris {
            for v in &[tri.a, tri.b, tri.c] {
                min.x = min.x.min(v.x);
                min.y = min.y.min(v.y);
                min.z = min.z.min(v.z);
                max.x = max.x.max(v.x);
                max.y = max.y.max(v.y);
                max.z = max.z.max(v.z);
            }
        }
        Aabb {
            min: min,
            max: max,
        }
    }

    pub fn with_min(&self, axis: usize, min: f32) -> Self {
        let mut new = *self;
        new.min[axis] = min;
        new
    }

    pub fn with_max(&self, axis: usize, max: f32) -> Self {
        let mut new = *self;
        new.max[axis] = max;
        new
    }

    pub fn intersect(&self, r: &Ray, t0: f32, t1: f32) -> Option<(f32, f32)> {
        // Williams, Amy, et al. "An efficient and robust ray-box intersection algorithm."
        // ACM SIGGRAPH 2005 Courses. ACM, 2005.
        let p = [self.min, self.max];
        let sign = [(r.d[0] < 0.0) as usize, (r.d[1] < 0.0) as usize, (r.d[2] < 0.0) as usize];
        let inv_dir = 1.0 / r.d;
        let mut tmin = (p[sign[0]].x - r.o.x) * inv_dir.x;
        let mut tmax = (p[1 - sign[0]].x - r.o.x) * inv_dir.x;
        let tymin = (p[sign[1]].y - r.o.y) * inv_dir.y;
        let tymax = (p[1 - sign[1]].y - r.o.y) * inv_dir.y;
        if tmin > tymax || tymin > tmax {
            return None;
        }
        tmin = tmin.min(tymin);
        tmax = tmax.max(tymax);
        let tzmin = (p[sign[2]].z - r.o.z) * inv_dir.z;
        let tzmax = (p[1 - sign[2]].z - r.o.z) * inv_dir.z;
        if tmin > tzmax || tzmin > tmax {
            return None;
        }
        tmin = tmin.min(tzmin);
        tmax = tmax.max(tzmax);
        if tmin < t1 && tmax > t0 {
            Some((tmin, tmax))
        } else {
            None
        }
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
