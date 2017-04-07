#![feature(conservative_impl_trait)]

extern crate arrayvec;
extern crate beebox;
extern crate beevage;
extern crate bmp;
extern crate cgmath;
#[macro_use]
extern crate clap;
extern crate cast;
extern crate elapsed;
#[macro_use]
extern crate lazy_static;
extern crate itertools;
extern crate obj;
extern crate ordered_float;
extern crate rayon;
extern crate regex;
extern crate watertri;

use cast::{usize, u32, f32, f64};
use cgmath::{InnerSpace, vec3};
use film::{Frame, Depthmap, Heatmap};
use geom::{Hit, Ray};
use rayon::prelude::*;
use scene::Scene;
use std::f32;
use std::path::PathBuf;
use std::time::Duration;

mod bvh;
mod cli;
mod film;
mod geom;
mod scene;

enum RenderKind {
    Depthmap,
    Heatmap,
}

pub struct Config {
    input_file: PathBuf,
    output_file: PathBuf,
    image_width: u32,
    image_height: u32,
    sah_buckets: u32,
    sah_traversal_cost: f32,
    num_threads: Option<u32>,
    render_kind: RenderKind,
}

fn primary_ray(x: u32, y: u32, cfg: &Config) -> Ray {
    let norm_x = (f32(x) + 0.5) / f32(cfg.image_width);
    let norm_y = (f32(y) + 0.5) / f32(cfg.image_height);
    let aspect_ratio = f32(cfg.image_width) / f32(cfg.image_height);
    let cam_x = aspect_ratio * (norm_x - 0.5);
    let cam_y = aspect_ratio * (0.5 - norm_y);
    let d = vec3(cam_x, cam_y, -1.0).normalize();
    Ray::new(vec3(0.0, 0.0, 0.0), d)
}

fn render<T, F>(scene: &Scene, cfg: &Config, background: T, shader: F) -> film::Frame<T>
    where F: Fn(&mut T, Hit, Ray) + Sync,
          T: Clone + Send + Sync
{
    let mut frame = Frame::new(cfg.image_width, cfg.image_height, background);
    frame.pixels_mut().for_each(|(x, y, px)| {
                                    let r = primary_ray(x, y, cfg);
                                    let hit = scene.intersect(&r);
                                    shader(px, hit, r);
                                });
    frame
}

fn render_depthmap(scene: &Scene, cfg: &Config) -> Box<film::ToBmp> {
    let frame = render(scene, cfg, f32::INFINITY, |px, hit, _| if hit.is_valid() {
        *px = hit.t;
    });
    Box::new(Depthmap(frame))
}

fn render_heatmap(scene: &Scene, cfg: &Config) -> Box<film::ToBmp> {
    let frame = render(scene, cfg, 0, |px, _, r| { *px = r.traversal_steps.get(); });
    Box::new(Heatmap(frame))
}

fn main() {
    let cfg = cli::parse_matches(cli::build_app().get_matches());
    if let Some(num_threads) = cfg.num_threads {
        let rayon_cfg = rayon::Configuration::new().num_threads(usize(num_threads));
        rayon::initialize(rayon_cfg).unwrap();
    }

    let scene = Scene::new(&cfg);
    let render: fn(_, _) -> _ = match cfg.render_kind {
        RenderKind::Depthmap => render_depthmap,
        RenderKind::Heatmap => render_heatmap,
    };
    let (frame, t) = measure_and_print_time("rendering", || render(&scene, &cfg));
    let output_file = cfg.output_file.display().to_string();
    print_timing("creating BMP",
                 move || frame.to_bmp().save(&output_file).unwrap());
    let rays_tested = scene.rays_tested();
    let seconds = f64(t.as_secs()) + f64(t.subsec_nanos()) / 1e9;
    let mrays = f64(rays_tested) / 1e6;
    let time_per_ray = t / u32(rays_tested).unwrap();
    println!("{:.2}M rays @ {:.3} Mray/s ({:} per ray)",
             mrays,
             mrays / seconds,
             elapsed::ElapsedDuration::new(time_per_ray));
}

fn measure_and_print_time<T, F>(description: &str, f: F) -> (T, Duration)
    where F: FnOnce() -> T
{
    let (t, result) = elapsed::measure_time(f);
    println!("[{:^10}] {}", t, description);
    (result, t.duration())
}

fn print_timing<T, F>(description: &str, f: F) -> T
    where F: FnOnce() -> T
{
    measure_and_print_time(description, f).0
}
