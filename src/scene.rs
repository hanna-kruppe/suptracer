use super::{Config, print_timing};
use bvh::{self, Bvh};
use cast::usize;
use cgmath::{Vector3, vec3};
use geom::{Hit, Ray, Tri, TriSliceExt};
use obj;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Scene {
    pub tris: Vec<Tri>,
    bvh: Bvh,
    rays_tested: AtomicUsize,
}

impl Scene {
    pub fn new(cfg: &Config) -> Self {
        let desc = format!("loading OBJ: {}", cfg.input_file.display());
        let mut tris = print_timing(&desc, || read_obj(&cfg.input_file));
        normalize(&mut tris);
        let (bvh, tris) = bvh::construct(&tris, cfg);
        Scene {
            tris,
            bvh,
            rays_tested: AtomicUsize::new(0),
        }
    }

    pub fn intersect(&self, r: &Ray) -> Hit {
        self.rays_tested.fetch_add(1, Ordering::SeqCst);
        bvh::traverse(&self.tris, &self.bvh, r)
    }

    pub fn rays_tested(&self) -> usize {
        self.rays_tested.load(Ordering::SeqCst)
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

fn read_obj(path: &Path) -> Vec<Tri> {
    let read = BufReader::new(File::open(path).unwrap());
    let o = obj::load_obj::<obj::Position, _>(read).unwrap();
    o.indices
        .chunks(3)
        .map(|chunk| {
            assert!(chunk.len() == 3);
            let (i, j, k) = (usize(chunk[0]), usize(chunk[1]), usize(chunk[2]));
            Tri {
                a: Vector3::from(o.vertices[i].position),
                b: Vector3::from(o.vertices[j].position),
                c: Vector3::from(o.vertices[k].position),
            }
        })
        .collect()
}
