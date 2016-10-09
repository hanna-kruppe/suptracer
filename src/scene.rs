use bvh::{self, Bvh};
use cgmath::{Vector3, vec3};
use film::Color;
use geom::{Hit, Ray, Tri, TriSliceExt};
use obj;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use super::{Config, timeit};

pub struct Scene {
    pub mesh: Mesh,
}

impl Scene {
    pub fn new(cfg: &Config) -> Self {
        Scene { mesh: read_obj(&cfg.input_file, cfg) }
    }
}

pub struct Mesh {
    pub tris: Vec<Tri>,
    accel: Bvh,
}

impl Mesh {
    fn new(mut tris: Vec<Tri>, cfg: &Config) -> Self {
        normalize(&mut tris);
        let (bvh, tris) = bvh::construct(&mut tris, cfg);
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
    let bb = tris.bbox();
    let (min, max) = (bb.min(), bb.max());
    let center = (min + max) / 2.0;
    // This heuristically moves the model such that it's probably within view.
    let displace = center + vec3(0.0, 0.0, (min.z - max.z).abs());
    for tri in tris {
        tri.a -= displace;
        tri.b -= displace;
        tri.c -= displace;
    }
}

fn read_obj(path: &Path, cfg: &Config) -> Mesh {
    const WHITE: Color = Color(255, 255, 255);
    let msg = format!("loading file: {}", path.display());
    let (tris, _) = timeit(&msg, || {
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
        tris
    });
    Mesh::new(tris, cfg)
}