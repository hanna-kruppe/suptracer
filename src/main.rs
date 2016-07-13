extern crate arrayvec;
extern crate bmp;
extern crate cgmath;
extern crate conv;
extern crate rayon;
extern crate scoped_threadpool;
extern crate stopwatch;
extern crate obj;
extern crate watertight_triangle;

use std::f32;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use cgmath::{InnerSpace, vec3};
use conv::prelude::*;
use scoped_threadpool::Pool;
use stopwatch::Stopwatch;

use geom::{Hit, Ray};
use scene::{Scene, Mesh, load_scene};

mod bb;
mod bvh;
mod geom;
mod scene;

#[derive(Copy, Clone, Debug)]
pub struct Color(u8, u8, u8);

static WIDTH: u32 = 1000;
static HEIGHT: u32 = 1000;

static BACKGROUND: Color = Color(0, 0, 255);

impl Color {
    fn to_px(self) -> bmp::Pixel {
        bmp::Pixel {
            r: self.0,
            g: self.1,
            b: self.2,
        }
    }
}

fn primary_ray(x: u32, y: u32) -> Ray {
    let norm_x = (x as f32 + 0.5) / (WIDTH as f32);
    let norm_y = (y as f32 + 0.5) / (HEIGHT as f32);
    let cam_x = 2.0 * norm_x - 1.0;
    let cam_y = 1.0 - 2.0 * norm_y;
    let d = vec3(cam_x, cam_y, -1.0).normalize();
    Ray::new(vec3(0.0, 0.0, 0.0), d)
}

fn trace(r: Ray, scene: &Scene) -> Color {
    let mut closest: Option<(&Mesh, Hit)> = None;
    for mesh in &scene.meshes {
        let hit = mesh.intersect(&r);
        if hit.is_valid() {
            closest = Some((mesh, hit));
        }
    }
    if let Some((mesh, hit)) = closest {
        let idx = hit.tri_id.value_as::<usize>().unwrap();
        mesh.tris[idx].color
    } else {
        BACKGROUND
    }
}


fn render(scene: Scene) -> u32 {
    let mut img = bmp::Image::new(WIDTH, HEIGHT);
    // TODO consider more coarse grained parallelism
    // 4 threads take 2.5 µs/ray, 1 thread takes 5 µs/ray
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
