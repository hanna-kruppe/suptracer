extern crate bmp;
extern crate cgmath;
extern crate scoped_threadpool;
extern crate obj;

use std::fs::File;
use std::f32;
use std::io::{BufReader, Write, stdout};
use std::sync::{Arc, Mutex};
use cgmath::{Vector3, InnerSpace, vec3};
use scoped_threadpool::Pool;

#[derive(Copy, Clone)]
struct Color(u8, u8, u8);

#[derive(Clone)]
struct Ray {
    o: Vector3<f32>,
    d: Vector3<f32>,
}

#[derive(Clone)]
struct Tri(Vector3<f32>, Vector3<f32>, Vector3<f32>, Color);

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

fn max_dim(v: Vector3<f32>) -> usize {
    let (x, y, z) = (v.x.abs(), v.y.abs(), v.z.abs());
    if x > y {
        if x > z {
            0
        } else {
            2
        }
    } else {
        if y > z {
            1
        } else {
            2
        }
    }
}

#[allow(non_snake_case)]
#[inline(never)]
fn intersect(tri: &Tri, r: &Ray) -> Option<f32> {
    let (org, dir) = (r.o, r.d);
    let kz = max_dim(dir);
    let (kx, ky) = if dir[kz] < 0.0 {
        ((kz + 1) % 3, (kz + 2) % 3)
    } else {
        ((kz + 2) % 3, (kz + 1) % 3)
    };

    let Sx = dir[kx] / dir[kz];
    let Sy = dir[ky] / dir[kz];
    let Sz = 1.0 / dir[kz];

    let (A, B, C) = match *tri {
        Tri(A, B, C, _) => (A - org, B - org, C - org),
    };
    let Ax = A[kx] - Sx * A[kz];
    let Ay = A[ky] - Sy * A[kz];
    let Bx = B[kx] - Sx * B[kz];
    let By = B[ky] - Sy * B[kz];
    let Cx = C[kx] - Sx * C[kz];
    let Cy = C[ky] - Sy * C[kz];

    let mut U = Cx * By - Cy * Bx;
    let mut V = Ax * Cy - Ay * Cx;
    let mut W = Bx * Ay - By * Ax;

    if U == 0. || V == 0. || W == 0. {
        let CxBy = (Cx as f64) * (By as f64);
        let CyBx = (Cy as f64) * (Bx as f64);
        U = (CxBy - CyBx) as f32;
        let AxCy = (Ax as f64) * (Cy as f64);
        let AyCx = (Ay as f64) * (Cx as f64);
        V = (AxCy - AyCx) as f32;
        let BxAy = (Bx as f64) * (Ay as f64);
        let ByAx = (By as f64) * (Ax as f64);
        W = (BxAy - ByAx) as f32;
    }

    if (U < 0. || V < 0. || W < 0.) && (U > 0. || V > 0. || W > 0.) {
        return None;
    }

    let det = U + V + W;
    if det == 0. {
        return None;
    }

    let Az = Sz * A[kz];
    let Bz = Sz * B[kz];
    let Cz = Sz * C[kz];
    let T = U * Az + V * Bz + W * Cz;

    Some(T / det)
}

fn trace(r: Ray, scene: &[Tri]) -> Color {
    let mut hit: Option<&Tri> = None;
    let mut t_closest_hit = f32::INFINITY;
    for o in scene.iter() {
        let t_hit;
        if let Some(t) = intersect(o, &r) {
            t_hit = t;
        } else {
            continue;
        }
        if t_hit < t_closest_hit {
            t_closest_hit = t_hit;
            hit = Some(o);
        }
    }
    match hit {
        Some(&Tri(_, _, _, color)) => color,
        None => BACKGROUND,
    }
}

fn normalize(vs: &mut [obj::Position]) {
    // First, compute an AABB for the model
    let mut min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = -min;
    for v in vs.iter() {
        min.x = min.x.min(v.position[0]);
        min.y = min.y.min(v.position[1]);
        min.z = min.z.min(v.position[2]);
        max.x = max.x.max(v.position[0]);
        max.y = max.y.max(v.position[1]);
        max.z = max.z.max(v.position[2]);
    }
    let center = (min + max) / 2.0;
    // This heuristically moves the model such that it's probably within view.
    let displace = center + vec3(0.0, 0.0, 1.5 * (min.z - max.z).abs());
    for v in vs {
        v.position[0] -= displace.x;
        v.position[1] -= displace.y;
        v.position[2] -= displace.z;
    }
}

fn read_obj() -> Vec<Tri> {
    let read = BufReader::new(File::open("bunny.obj").unwrap());
    let mut o = obj::load_obj::<obj::Position, _>(read).unwrap();
    normalize(&mut o.vertices);
    let mut tris = Vec::with_capacity(o.indices.len() / 3);
    for chunk in o.indices.chunks(3) {
        assert!(chunk.len() == 3);
        let i = chunk[0];
        let j = chunk[1];
        let k = chunk[2];
        let tri = Tri(Vector3::from(o.vertices[i as usize].position),
                      Vector3::from(o.vertices[j as usize].position),
                      Vector3::from(o.vertices[k as usize].position),
                      WHITE);
        tris.push(tri);
    }
    tris
}

fn main() {
    let scene = read_obj();
    let mut img = bmp::Image::new(WIDTH, HEIGHT);
    let mut pool = Pool::new(4);
    pool.scoped(|scope| {
        let coords = img.coordinates();
        let img_root = Arc::new(Mutex::new(&mut img));
        for (i, j) in coords {
            let my_img = img_root.clone();
            let scene = &scene;
            scope.execute(move || {
                let traced = trace(primary_ray(i, j), scene);
                let mut img = my_img.lock().unwrap();
                img.set_pixel(i, j, traced.to_px());
                if i == 0 {
                    let out = stdout();
                    let mut out = out.lock();
                    out.write(b".").unwrap();
                    out.flush().unwrap();
                }
            });
        }
    });
    img.save("bunny.bmp").unwrap();
}
