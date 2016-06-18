extern crate bmp;
extern crate cgmath;
extern crate conv;
// extern crate rayon;
extern crate scoped_threadpool;
extern crate stopwatch;
extern crate obj;
extern crate watertight_triangle;

mod bb;
mod bvh;

use bb::Aabb;
use bvh::Bvh;
use std::f32;
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use std::path::Path;
use std::sync::{Arc, Mutex};
use cgmath::{Vector3, InnerSpace, vec3};
use scoped_threadpool::Pool;
use stopwatch::Stopwatch;

#[derive(Copy, Clone, Debug)]
pub struct Color(u8, u8, u8);

#[derive(Debug)]
pub struct Ray {
    pub o: Vector3<f32>,
    pub d: Vector3<f32>,
}

struct Mesh {
    tris: Vec<Tri>,
    bb: Aabb,
    accel: Bvh,
}

#[derive(Copy, Clone, Debug)]
pub struct Tri {
    pub a: Vector3<f32>,
    pub b: Vector3<f32>,
    pub c: Vector3<f32>,
    pub color: Color,
}

pub struct Hit<'a> {
    pub t: f32,
    pub tri: &'a Tri,
}

impl<'a> Hit<'a> {
    fn new(t: f32, tri: &'a Tri) -> Hit<'a> {
        Hit { t: t, tri: tri }
    }
}

struct Scene {
    pub meshes: Vec<Mesh>,
}

static WIDTH: u32 = 500;
static HEIGHT: u32 = 500;

static BACKGROUND: Color = Color(0, 0, 255);
static WHITE: Color = Color(255, 255, 255);

impl Color {
    fn to_px(self) -> bmp::Pixel {
        bmp::Pixel {
            r: self.0,
            g: self.1,
            b: self.2,
        }
    }
}

impl Mesh {
    fn new(mut tris: Vec<Tri>) -> Self {
        normalize(&mut tris);
        let bb = Aabb::new(&tris);
        let bvh = bvh::construct(&mut tris, bb.clone());
        Mesh {
            tris: tris,
            bb: bb,
            accel: bvh,
        }
    }

    fn intersect<'a>(&'a self, r: &Ray) -> Option<Hit<'a>> {
        if let Some(tmax) = self.bb.intersect(r, f32::INFINITY) {
            bvh::traverse(&self.tris, &self.accel, r, tmax)
        } else {
            None
        }
    }
}

fn primary_ray(x: u32, y: u32) -> Ray {
    let norm_x = (x as f32 + 0.5) / (WIDTH as f32);
    let norm_y = (y as f32 + 0.5) / (HEIGHT as f32);
    let cam_x = 2.0 * norm_x - 1.0;
    let cam_y = 1.0 - 2.0 * norm_y;
    let d = vec3(cam_x, cam_y, -1.0).normalize();
    return Ray {
        o: vec3(0.0, 0.0, 0.0),
        d: d,
    };
}

fn intersect<'a>(tris: &'a [Tri], ray: &Ray) -> Option<Hit<'a>> {
    let mut closest: Option<Hit> = None;
    for tri in tris {
        let corners = (tri.a, tri.b, tri.c);
        if let Some(t) = watertight_triangle::intersect(corners, ray.o, ray.d) {
            if closest.is_none() || t < closest.as_ref().unwrap().t {
                closest = Some(Hit::new(t, tri));
            }
        }
    }
    closest
}

fn trace(r: Ray, scene: &Scene) -> Color {
    let mut closest: Option<Hit> = None;
    for mesh in &scene.meshes {
        if let Some(hit) = mesh.intersect(&r) {
            if closest.is_none() || hit.t < closest.as_ref().unwrap().t {
                closest = Some(hit);
            }
        }
    }
    match closest {
        Some(hit) => hit.tri.color,
        None => BACKGROUND,
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

fn load_scene() -> Scene {
    Scene { meshes: vec![read_obj("bunny.obj")] }
}

fn render(scene: Scene) -> u32 {
    let mut img = bmp::Image::new(WIDTH, HEIGHT);
    let mut pool = Pool::new(4);
    let mut ray_count = 0;
    pool.scoped(|scope| {
        let coords = img.coordinates();
        let img_root = Arc::new(Mutex::new(&mut img));
        for (i, j) in coords {
            let my_img = img_root.clone();
            let scene = &scene;
            ray_count += 1;
            scope.execute(move || {
                let traced = trace(primary_ray(i, j), scene);
                let mut img = my_img.lock().unwrap();
                img.set_pixel(i, j, traced.to_px());
            });
        }
    });
    img.save("bunny.bmp").unwrap();
    ray_count
}

fn main() {
    let (scene, _) = timeit("loaded scene", load_scene);
    let (ray_count, t) = timeit("traced rays", move || render(scene));
    let seconds = t.as_secs() as f64 + (t.subsec_nanos() as f64 / 1e9);
    let mrays = ray_count as f64 / 1e6;
    println!("{:.2}M rays @ {:.3} Mray/s ({} per ray)",
             mrays,
             mrays / seconds,
             pretty_duration(t / ray_count));
}

fn pretty_duration(d: Duration) -> String {
    if d.as_secs() > 0 {
        let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
        return format!("{:.2}s", secs);
    }
    let ns = d.subsec_nanos();
    if ns > 1_000_000 {
        return format!("{:.2}ms", ns as f64 / 1e6);
    } else if ns > 1_000 {
        return format!("{:.2}µs", ns as f64 / 1e3);
    } else {
        return format!("{}ns", ns);
    }
}

fn timeit<T, F>(description: &str, f: F) -> (T, Duration)
    where F: FnOnce() -> T
{
    let sw = Stopwatch::start_new();
    let result = f();
    let t = sw.elapsed();
    println!("{} in {}", description, pretty_duration(t));
    (result, t)
}
