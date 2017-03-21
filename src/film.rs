use bmp;
use cast::{usize, u32};
use rayon::prelude::*;
use std::cmp;
use std::f32;

pub struct Frame<T> {
    width: u32,
    height: u32,
    buffer: Vec<T>,
}

impl<T: Clone> Frame<T> {
    pub fn new(width: u32, height: u32, value: T) -> Self {
        Frame {
            width: width,
            height: height,
            buffer: vec![value; usize(width) * usize(height)],
        }
    }
}

// False positive in a clippy lint, see Manishearth/rust-clippy#740
// TODO fix https://github.com/Manishearth/rust-clippy/issues/1133 so this can be a cfg_attr
#[allow(unknown_lints, needless_lifetimes)]
impl<T: Sync + Send> Frame<T> {
    pub fn pixels<'a>(&'a self) -> impl IndexedParallelIterator<Item = (u32, u32, &'a T)> {
        // TODO why is this height and not width?
        let height = self.height;
        self.buffer[..]
            .par_iter()
            .enumerate()
            // TODO iterate differently to avoid the divmod
            .map(move |(i, px)| (u32(i).unwrap() / height, u32(i).unwrap() % height, px))
    }

    pub fn pixels_mut<'a>(&'a mut self)
                          -> impl IndexedParallelIterator<Item = (u32, u32, &'a mut T)> {
        let height = self.height;
        self.buffer[..]
            .par_iter_mut()
            .enumerate()
            // TODO iterate differently to avoid the divmod
            .map(move |(i, px)| (u32(i).unwrap() / height, u32(i).unwrap() % height, px))
    }
}

/// Compute the linear interpolation coefficient for producing x from x0 and x1, i.e.,
/// the scalar t \in [0, 1] such that x = (1 - t) * x0 + t * x1
/// Panics if this is not possible, i.e., x is not between x0 and x1.
fn inv_lerp<T: Copy + Into<f64> + PartialOrd>(x: T, x0: T, x1: T) -> f64 {
    assert!(x0 <= x && x <= x1);
    (x.into() - x0.into()) / (x1.into() - x0.into())
}

pub trait ToBmp {
    fn to_bmp(&self) -> bmp::Image;
}

pub struct Depthmap(pub Frame<f32>);
pub struct Heatmap(pub Frame<u32>);

impl ToBmp for Depthmap {
    fn to_bmp(&self) -> bmp::Image {
        let Frame { ref buffer, width, height } = self.0;
        let mut img = bmp::Image::new(width, height);
        let min_depth = buffer.iter().cloned().fold(f32::INFINITY, f32::min);
        assert!(min_depth != f32::INFINITY);
        // inf is background, filter it out and give it a special color
        let max_depth = buffer.iter()
            .cloned()
            .filter(|x| !x.is_infinite())
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_depth != f32::INFINITY);
        // FIXME .collect() shouldn't be necessary
        for (x, y, &depth) in self.0.pixels().collect::<Vec<_>>() {
            let color = if depth == f32::INFINITY {
                bmp::Pixel {
                    r: 0,
                    g: 0,
                    b: 255,
                }
            } else {
                let intensity = inv_lerp(depth, min_depth, max_depth);
                debug_assert!(0.0 <= intensity && intensity <= 1.0);
                let s = ((1.0 - intensity) * 255.0).round() as u8;
                bmp::Pixel { r: s, g: s, b: s }
            };
            img.set_pixel(x, y, color);
        }
        img
    }
}

impl ToBmp for Heatmap {
    fn to_bmp(&self) -> bmp::Image {
        let Frame { ref buffer, width, height } = self.0;
        let mut sorted = buffer.clone();
        sorted.sort();
        let pct05 = sorted[sorted.len() * 5 / 100];
        let pct95 = sorted[sorted.len() * 95 / 100];
        let mut img = bmp::Image::new(width, height);
        // FIXME .collect() shouldn't be necessary
        for (x, y, &heat) in self.0.pixels().collect::<Vec<_>>() {
            let clamped_heat = cmp::min(cmp::max(heat, pct05), pct95);
            let intensity = inv_lerp(clamped_heat, pct05, pct95);
            debug_assert!(0.0 <= intensity && intensity <= 1.0);
            let s = (intensity * 255.0).round() as u8;
            img.set_pixel(x, y, bmp::Pixel { r: s, g: 0, b: 0 });
        }
        img
    }
}
