use bmp;
use cast::{usize, u32};
use rayon::prelude::*;

#[derive(Copy, Clone, Debug)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub fn to_px(self) -> bmp::Pixel {
        bmp::Pixel {
            r: self.0,
            g: self.1,
            b: self.2,
        }
    }
}

pub struct Frame<T> {
    width: u32,
    height: u32,
    buffer: Box<[T]>,
}

impl<T: Clone> Frame<T> {
    pub fn new(width: u32, height: u32, value: T) -> Self {
        let buf = vec![value; usize(width) * usize(height)];
        Frame {
            width: width,
            height: height,
            buffer: buf.into_boxed_slice(),
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
            .map(move |(i, px)| (u32(i).unwrap() / height, u32(i).unwrap() % height, px))
    }

    pub fn pixels_mut<'a>(&'a mut self)
                          -> impl IndexedParallelIterator<Item = (u32, u32, &'a mut T)> {
        let height = self.height;
        self.buffer[..]
            .par_iter_mut()
            .enumerate()
            .map(move |(i, px)| (u32(i).unwrap() / height, u32(i).unwrap() % height, px))
    }
}

#[allow(dead_code)]
impl Frame<Color> {
    pub fn to_bmp(&self) -> bmp::Image {
        let mut img = bmp::Image::new(self.width, self.height);
        // FIXME .collect() shouldn't be necessary
        for (x, y, color) in self.pixels().collect::<Vec<_>>() {
            img.set_pixel(x, y, color.to_px());
        }
        img
    }
}

// Integer quantities are generally used for heat maps (e.g., traversal steps in BVH)
#[allow(dead_code)]
impl Frame<u32> {
    pub fn to_bmp(&self) -> bmp::Image {
        let count = self.buffer.len();
        let sorted = {
            let mut v = self.buffer.clone();
            v.sort();
            v
        };
        let pct05 = sorted[count * 5 / 100];
        let pct95 = sorted[count * 95 / 100];
        println!("BVH traversal steps: 5th percentile={} / 95th percentile={}",
                 pct05,
                 pct95);
        let mut img = bmp::Image::new(self.width, self.height);
        // FIXME .collect() shouldn't be necessary
        for (x, y, heat) in self.pixels().collect::<Vec<_>>() {
            let intensity = (*heat - pct05) as f64 / (pct95 - pct05) as f64;
            let intensity = intensity.max(0.0).min(1.0);
            debug_assert!(0.0 <= intensity && intensity <= 1.0);
            let quantized_intensity = (intensity * 255.0).round() as u8;
            img.set_pixel(x, y, Color(quantized_intensity, 0, 0).to_px());
        }
        img
    }
}
