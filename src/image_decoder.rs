use image::{GenericImage, Rgb, RgbImage};

struct PixelAverage {
    avg_rb: u32,
    avg_g: u32,
}

impl PixelAverage {
    pub fn new() -> PixelAverage {
        PixelAverage {
            avg_rb: 0,
            avg_g: 0,
        }
    }

    pub fn add(&mut self, rgb: u32) {
        let rb = rgb & 0x00FF00FF;
        let g = rgb & 0x0000FF00;
        self.avg_rb += rb;
        self.avg_g += g;
    }

    pub fn rgb(self) -> Rgb<u8> {
        let rb = self.avg_rb / 16;
        let g = (self.avg_g / 16) >> 8;
        let b = rb;
        let r = rb >> 16;

        Rgb([r as _, g as _, b as _])
    }
}

pub trait ToRgb {
    fn rgb(&self) -> Rgb<u8>;
}

pub struct RgbPixel {
    dat: [u8; 3],
}

impl RgbPixel {
    pub fn new(r: u8, g: u8, b: u8) -> RgbPixel {
        RgbPixel { dat: [r, g, b] }
    }
}

impl ToRgb for RgbPixel {
    fn rgb(&self) -> Rgb<u8> {
        Rgb(self.dat)
    }
}

pub struct YUV420Pixel {
    dat: [u8; 3],
}

impl YUV420Pixel {
    pub fn new(c: u8, d: u8, e: u8) -> YUV420Pixel {
        YUV420Pixel { dat: [c, d, e] }
    }
}

impl ToRgb for YUV420Pixel {
    fn rgb(&self) -> Rgb<u8> {
        let y = self.dat[0] as i32;
        let u = self.dat[1] as i32;
        let v = self.dat[2] as i32;
        let c = y - 16;
        let d = u - 128;
        let e = v - 128;

        let r = (298 * c + 409 * e + 128) >> 8;
        let g = (298 * c - 100 * d - 208 * e + 128) >> 8;
        let b = (298 * c + 516 * d + 128) >> 8;

        let clamp = |v| {
            if v > 0xFF {
                0xFF
            } else {
                if v < 0 {
                    0
                } else {
                    v as u8
                }
            }
        };

        Rgb([clamp(r), clamp(g), clamp(b)])
    }
}

pub struct Rgb565 {
    dat: u16,
}

impl Rgb565 {
    fn new(dat: u16) -> Self {
        Rgb565 { dat }
    }
}

impl ToRgb for Rgb565 {
    fn rgb(&self) -> Rgb<u8> {
        let byte = |i, c| (((1 << c) - 1) as u16 & (self.dat >> i)) as u8;

        let r8 = (byte(11, 5) as u16 * 527 + 23) >> 6;
        let g8 = (byte(5, 6) as u16 * 259 + 33) >> 6;
        let b8 = (byte(0, 5) as u16 * 527 + 23) >> 6;
        Rgb([r8 as u8, g8 as u8, b8 as u8])
    }
}

pub fn rgb565_to_rgb888(mapping: &[u16], pitch: u32, size: (u32, u32)) -> RgbImage {
    let mut img = RgbImage::new(size.0, size.1);

    let bytepitch = pitch / 2;

    for y in 0..size.1 {
        for x in 0..size.0 {
            let offset = y * bytepitch + x;
            let v = Rgb565::new(mapping[offset as usize]);

            unsafe { img.unsafe_put_pixel(x, y, v.rgb()) };
        }
    }
    img
}

pub fn decode_image(mapping: &[u32], pitch: u32, size: (u32, u32)) -> RgbImage {
    let mut img = RgbImage::new(size.0, size.1);

    let bytepitch = pitch / 4;

    for y in 0..size.1 {
        for x in 0..size.0 {
            let offset = y * bytepitch + x;
            let v = mapping[offset as usize];
            let byte = |i| (v >> i * 8) as u8;

            let px = Rgb([byte(2), (byte(1)), (byte(0))]);

            unsafe { img.unsafe_put_pixel(x, y, px) };
        }
    }

    img
}

pub fn decode_image_multichannel(
    mappings: [&[u8]; 3],
    size: (u32, u32),
    pitches: [u32; 3],
) -> RgbImage {
    let mut img = RgbImage::new(size.0, size.1);

    for y in 0..size.1 {
        for x in 0..size.0 {
            let offset: usize = (y * pitches[0] + x) as _;
            let offset1: usize = ((y / 2) * (pitches[1]) + x / 2) as _;
            let offset2: usize = ((y / 2) * (pitches[2]) + x / 2) as _;
            let yuv = YUV420Pixel::new(
                mappings[0][offset],
                mappings[1][offset1],
                mappings[2][offset2],
            );

            unsafe { img.unsafe_put_pixel(x, y, yuv.rgb()) };
        }
    }

    img
}

pub fn decode_small_image_multichannel(
    mappings: [&[u8]; 3],
    size: (u32, u32),
    pitches: [u32; 3],
) -> RgbImage {
    let halfsize = (size.0 / 2, size.1 / 2);
    let mut img = RgbImage::new(halfsize.0, halfsize.1);

    for y in 0..halfsize.1 {
        for x in 0..halfsize.0 {
            let offset: usize = (2 * y * pitches[0] + 2 * x) as _;
            let offset1: usize = (y * pitches[1] + x) as _;
            let offset2: usize = (y * pitches[2] + x) as _;
            let yat = |offset| mappings[0][offset] as u32;
            let yval = (yat(offset)
                + yat(offset + 1)
                + yat(offset + pitches[0] as usize)
                + yat(offset + pitches[0] as usize + 1))
                / 4;
            let yuv = YUV420Pixel::new(yval as _, mappings[1][offset1], mappings[2][offset2]);

            unsafe { img.unsafe_put_pixel(x, y, yuv.rgb()) };
        }
    }

    img
}

pub fn decode_tiled_small_image(
    mapping: &[u32],
    tilesize: u32,
    tiles: (u32, u32),
    size: (u32, u32),
) -> RgbImage {
    let mut img = RgbImage::new(tiles.0 * tilesize / 4, tiles.1 * tilesize / 4);

    let mut i = 0;

    let mut avg_16 = |x, y| {
        let mut avg = PixelAverage::new();
        for n in 0..16 {
            avg.add(mapping[i + n]);
        }
        unsafe {
            img.unsafe_put_pixel(x, y, avg.rgb());
        }
        i = i + 16;
    };

    let mut copy_16x4_px = |x, y| {
        avg_16(x, y);
        avg_16(x + 1, y);
        avg_16(x + 2, y);
        avg_16(x + 3, y);
    };

    let mut copy_16x16_px = |x, y| {
        copy_16x4_px(x, y);
        copy_16x4_px(x, y + 1);
        copy_16x4_px(x, y + 2);
        copy_16x4_px(x, y + 3);
    };

    for ytile in 0..tiles.1 {
        if ytile % 2 == 0 {
            let mut copy_tile = |x, y| {
                copy_16x16_px(x, y);
                copy_16x16_px(x, y + 4);
                copy_16x16_px(x + 4, y + 4);
                copy_16x16_px(x + 4, y);
            };

            for xtile in 0..tiles.0 {
                copy_tile(xtile * tilesize / 4, ytile * tilesize / 4);
            }
        } else {
            let mut copy_tile = |x, y| {
                copy_16x16_px(x + 4, y + 4);
                copy_16x16_px(x + 4, y);
                copy_16x16_px(x, y);
                copy_16x16_px(x, y + 4);
            };

            for xtile in (0..tiles.0).rev() {
                copy_tile(xtile * tilesize / 4, ytile * tilesize / 4);
            }
        }
    }

    img.sub_image(0, 0, size.0 / 4, size.1 / 4).to_image()
}

pub fn to_image(mapping: &[u8], tilesize: u32, tiles: (u32, u32), size: (u32, u32)) -> RgbImage {
    let mut img = RgbImage::new(tiles.0 * tilesize, tiles.1 * tilesize);
    let mut i = 0;

    let mut copy_px = |x, y| {
        let color = Rgb([
            mapping[(i + 2) as usize],
            mapping[(i + 1) as usize],
            mapping[(i + 0) as usize],
        ]);
        unsafe {
            img.unsafe_put_pixel(x, y, color);
        }
        i = i + 4;
    };
    let mut copy_4_px = |x, y| {
        copy_px(x, y);
        copy_px(x + 1, y);
        copy_px(x + 2, y);
        copy_px(x + 3, y);
    };

    let mut copy_4x4_px = |x, y| {
        copy_4_px(x, y);
        copy_4_px(x, y + 1);
        copy_4_px(x, y + 2);
        copy_4_px(x, y + 3);
    };

    let mut copy_16x4_px = |x, y| {
        copy_4x4_px(x, y);
        copy_4x4_px(x + 4, y);
        copy_4x4_px(x + 8, y);
        copy_4x4_px(x + 12, y);
    };

    let mut copy_16x16_px = |x, y| {
        copy_16x4_px(x, y);
        copy_16x4_px(x, y + 4);
        copy_16x4_px(x, y + 8);
        copy_16x4_px(x, y + 12);
    };

    for ytile in 0..tiles.1 {
        if ytile % 2 == 0 {
            let mut copy_tile = |x, y| {
                copy_16x16_px(x, y);
                copy_16x16_px(x, y + 16);
                copy_16x16_px(x + 16, y + 16);
                copy_16x16_px(x + 16, y);
            };

            for xtile in 0..tiles.0 {
                copy_tile(xtile * tilesize, ytile * tilesize);
            }
        } else {
            let mut copy_tile = |x, y| {
                copy_16x16_px(x + 16, y + 16);
                copy_16x16_px(x + 16, y);
                copy_16x16_px(x, y);
                copy_16x16_px(x, y + 16);
            };

            for xtile in (0..tiles.0).rev() {
                copy_tile(xtile * tilesize, ytile * tilesize);
            }
        }
    }

    img.sub_image(0, 0, size.0, size.1).to_image()
}
