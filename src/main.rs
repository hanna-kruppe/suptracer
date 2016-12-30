#![feature(conservative_impl_trait)]

extern crate arrayvec;
extern crate beebox;
extern crate beevage;
extern crate bmp;
extern crate cgmath;
#[macro_use]
extern crate clap;
extern crate cast;
#[macro_use]
extern crate lazy_static;
extern crate obj;
extern crate rayon;
extern crate regex;
extern crate watertri;

use cast::{usize, u32};
use cgmath::{InnerSpace, vec3};

use film::{Frame, Color};
use geom::Ray;
use rayon::prelude::*;
use scene::Scene;
use std::f32;
use std::path::PathBuf;
use std::time::Instant;

use std::sync::atomic::Ordering;
use std::time::Duration;

mod bvh;
mod cli;
mod film;
mod geom;
mod scene;

pub struct Config {
    input_file: PathBuf,
    output_file: PathBuf,
    image_width: u32,
    image_height: u32,
    sah_buckets: u32,
    sah_traversal_cost: f32,
    num_threads: Option<u32>,
    heatmap: bool,
}

fn primary_ray(x: u32, y: u32, cfg: &Config) -> Ray {
    let norm_x = (x as f32 + 0.5) / (cfg.image_width as f32);
    let norm_y = (y as f32 + 0.5) / (cfg.image_height as f32);
    let cam_x = 2.0 * norm_x - 1.0;
    let cam_y = 1.0 - 2.0 * norm_y;
    let d = vec3(cam_x, cam_y, -1.0).normalize();
    Ray::new(vec3(0.0, 0.0, 0.0), d)
}

fn render(scene: &Scene, cfg: &Config) -> Frame<Color> {
    const BACKGROUND: Color = Color(0, 0, 255);

    let mut frame = Frame::new(cfg.image_width, cfg.image_height, BACKGROUND);
    frame.pixels_mut().for_each(|(x, y, px)| {
        let r = primary_ray(x, y, cfg);
        let hit = scene.intersect(&r);
        if hit.is_valid() {
            *px = scene.mesh.tris[usize(hit.tri_id)].color;
        } else {
            *px = BACKGROUND;
        }
    });
    frame
}

fn render_heatmap(scene: &Scene, cfg: &Config) -> Frame<u32> {
    let mut frame = Frame::new(cfg.image_width, cfg.image_height, 0);
    frame.pixels_mut().for_each(|(x, y, px)| {
        let r = primary_ray(x, y, cfg);
        scene.intersect(&r);
        *px = r.traversal_steps.get();
    });
    frame
}

fn pretty_duration(d: Duration) -> String {
    if d.as_secs() > 0 {
        let secs = d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9;
        return format!("{:>6.2}s ", secs);
    }
    let ns = d.subsec_nanos();
    if ns > 1_000_000 {
        return format!("{:>6.2}ms", ns as f64 / 1e6);
    } else if ns > 1_000 {
        return format!("{:>6.2}µs", ns as f64 / 1e3);
    } else {
        return format!("{:>6}ns", ns);
    }
}

fn timeit<T, F>(description: &str, f: F) -> (T, Duration)
    where F: FnOnce() -> T
{
    let t0 = Instant::now();
    let result = f();
    let t = Instant::now() - t0;
    println!("[{}] {}", pretty_duration(t), description);
    (result, t)
}

fn main() {
    let cfg = cli::parse_matches(cli::build_app().get_matches());
    if let Some(num_threads) = cfg.num_threads {
        rayon::initialize(rayon::Configuration::new().set_num_threads(usize(num_threads))).unwrap();
    }

    let scene = Scene::new(&cfg);
    let output_file = cfg.output_file.display().to_string();
    let t;
    if cfg.heatmap {
        let (frame, render_time) = timeit("traced rays", || render_heatmap(&scene, &cfg));
        t = render_time;
        timeit("wrote render",
               move || frame.to_bmp().save(&output_file).unwrap());
    } else {
        let (frame, render_time) = timeit("traced rays", || render(&scene, &cfg));
        t = render_time;
        timeit("wrote heatmap",
               move || frame.to_bmp().save(&output_file).unwrap());

    }
    let rays_tested = u32(scene.rays_tested.load(Ordering::SeqCst)).unwrap();
    let seconds = t.as_secs() as f64 + (t.subsec_nanos() as f64 / 1e9);
    let mrays = rays_tested as f64 / 1e6;
    println!("{:.2}M rays @ {:.3} Mray/s ({} per ray)",
             mrays,
             mrays / seconds,
             pretty_duration(t / rays_tested));
}
