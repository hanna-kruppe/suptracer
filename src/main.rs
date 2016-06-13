extern crate bmp;
extern crate cgmath;
extern crate conv;
extern crate scoped_threadpool;
extern crate stopwatch;
extern crate obj;
extern crate watertight_triangle;

use std::fmt;
use std::fs::File;
use std::f32;
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
    pub tris: Vec<Tri>,
    pub bb: Aabb,
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

#[derive(Copy, Clone, Debug)]
pub struct Aabb {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
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
        let bb = compute_aabb(&tris);
        Mesh {
            tris: tris,
            bb: bb,
        }
    }
    fn intersect<'a>(&'a self, r: &Ray) -> Option<Hit<'a>> {
        intersect(&self.tris, r)
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

pub fn compute_aabb(tris: &[Tri]) -> Aabb {
    let mut min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = -min;
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

fn normalize(tris: &mut [Tri]) {
    let Aabb { min, max } = compute_aabb(tris);
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

fn main() {
    let sw = Stopwatch::start_new();
    let scene = load_scene();
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
    println!("");
    let elapsed = sw.elapsed();
    println!("Traced {:.2}M rays in {} ({} per ray)",
             ray_count as f64 / 1e6,
             pretty_duration(elapsed),
             pretty_duration(elapsed / ray_count));
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
