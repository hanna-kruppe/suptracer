use bmp;
use cast::usize;
use itertools::Itertools;

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

impl<T> Frame<T> {
    // rustfmt 0.6.2 breaks -> impl Trait by replacing "Trait" with "TODO"
    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub fn pixels<'a>(&'a mut self) -> impl Iterator<Item=((u32, u32), &'a mut T)> {
        (0..self.width).cartesian_product(0..self.height).zip(self.buffer.iter_mut())
    }
}

#[allow(dead_code)]
impl Frame<Color> {
    pub fn to_bmp(&mut self) -> bmp::Image {
        let mut img = bmp::Image::new(self.width, self.height);
        for ((i, j), color) in self.pixels() {
            img.set_pixel(i, j, color.to_px());
        }
        img
    }
}

// Integer quantities are generally used for heat maps (e.g., traversal steps in BVH)
#[allow(dead_code)]
impl Frame<u32> {
    pub fn to_bmp(&mut self) -> bmp::Image {
        let min = self.buffer.iter().cloned().min().unwrap();
        let max = self.buffer.iter().cloned().max().unwrap();
        let mut img = bmp::Image::new(self.width, self.height);
        for ((i, j), heat) in self.pixels() {
            let intensity = (*heat - min) as f64 / (max - min) as f64;
            debug_assert!(0.0 <= intensity && intensity <= 1.0);
            let quantized_intensity = (intensity * 255.0).round() as u8;
            img.set_pixel(i, j, Color(quantized_intensity, 0, 0).to_px());
        }
        img
    }
}
