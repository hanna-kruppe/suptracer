use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use cgmath::{Vector3, vec3};
use obj;

use super::Color;
use bb::Aabb;
use bvh::{self, Bvh};
use geom::{Hit, Tri, Ray};

pub struct Scene {
    pub meshes: Vec<Mesh>,
}

pub struct Mesh {
    pub tris: Vec<Tri>,
    accel: Bvh,
}

impl Mesh {
    fn new(mut tris: Vec<Tri>) -> Self {
        normalize(&mut tris);
        let bb = Aabb::new(&tris);
        let bvh = bvh::construct(&mut tris, bb.clone());
        Mesh {
            tris: tris,
            accel: bvh,
        }
    }

    pub fn intersect(&self, r: &Ray) -> Hit {
        bvh::traverse(&self.tris, &self.accel, r)
    }
}

fn normalize(tris: &mut [Tri]) {
    let Aabb { min, max } = Aabb::new(tris);
    let center = (min + max) / 2.0;
    // This heuristically moves the model such that it's probably within view.
    let displace = center + vec3(0.0, 0.0, 1.5 * (min.z - max.z).abs());
    for tri in tris {
        tri.a -= displace;
        tri.b -= displace;
        tri.c -= displace;
    }
}

fn read_obj<P: AsRef<Path>>(path: P) -> Mesh {
    const WHITE: Color = Color(255, 255, 255);
    let read = BufReader::new(File::open(path).unwrap());
    let o = obj::load_obj::<obj::Position, _>(read).unwrap();
    let mut tris = Vec::with_capacity(o.indices.len() / 3);
    for chunk in o.indices.chunks(3) {
        assert!(chunk.len() == 3);
        let (i, j, k) = (chunk[0] as usize, chunk[1] as usize, chunk[2] as usize);
        tris.push(Tri {
            a: Vector3::from(o.vertices[i].position),
            b: Vector3::from(o.vertices[j].position),
            c: Vector3::from(o.vertices[k].position),
            color: WHITE,
        });
    }
    Mesh::new(tris)
}

pub fn load_scene() -> Scene {
    Scene { meshes: vec![read_obj("bunny.obj")] }
}
