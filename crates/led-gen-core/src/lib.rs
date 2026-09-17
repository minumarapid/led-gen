#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use serde::Deserialize;
use thiserror::Error;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};
use tsify::Tsify;
use wasm_bindgen::prelude::*;

pub const DEFAULT_BORDER: u32 = 10;
pub const DEFAULT_LED_SIZE: u32 = 4;
pub const DEFAULT_LED_GAP: u32 = 2;
pub const DEFAULT_LED_EXPOSURE: f32 = 1.0;
pub const DEFAULT_ENABLE_GLOW: bool = true;
pub const DEFAULT_GLOW_RANGE: f32 = 3.0;
pub const DEFAULT_GLOW_STRENGTH: f32 = 1.75;
pub const DEFAULT_GLOW_EXPOSURE: f32 = 1.0;
pub const DEFAULT_OFF_LIGHT_COLOR: [u8; 3] = [64, 64, 64];

pub const DEFAULT_CANVAS_BACKGROUND: [u8; 3] = [16, 16, 16];

pub const DEFAULT_LED_SHAPE: LedShape = LedShape::Circle;

#[cfg(feature = "cli")]
pub fn parse_off_light_color(value: &str) -> Result<[u8; 3], String> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 3 {
        return Err("R,G,B の 3 要素を指定してください (例: 50,50,50)".to_string());
    }
    let mut parsed = [0u8; 3];
    for (idx, part) in parts.iter().enumerate() {
        parsed[idx] = part
            .trim()
            .parse::<u8>()
            .map_err(|_| "0-255 の数値で指定してください".to_string())?;
    }
    Ok(parsed)
}

#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Clone, Debug, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
#[tsify(from_wasm_abi)]
pub enum LedShape {
    #[serde(alias = "Square")]
    Square,
    #[serde(alias = "Circle")]
    Circle,
}

#[cfg_attr(feature = "cli", derive(clap::Args))]
#[derive(Clone, Debug, Deserialize, Tsify)]
#[serde(default, rename_all = "camelCase")]
#[tsify(from_wasm_abi)]
pub struct LedConfig {
    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_BORDER))]
    pub border: u32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_LED_SIZE))]
    #[serde(alias = "led_size")]
    pub led_size: u32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_LED_GAP))]
    #[serde(alias = "led_gap")]
    pub led_gap: u32,

    #[cfg_attr(feature = "cli", arg(long, value_enum, default_value_t = DEFAULT_LED_SHAPE))]
    #[serde(alias = "led_shape")]
    pub led_shape: LedShape,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_LED_EXPOSURE))]
    #[serde(alias = "led_exposure")]
    pub led_exposure: f32,

    #[cfg_attr(feature = "cli", arg(long = "no-glow", action = clap::ArgAction::SetFalse))]
    #[serde(alias = "enable_glow")]
    pub enable_glow: bool,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_GLOW_RANGE))]
    #[serde(alias = "glow_range")]
    pub glow_range: f32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_GLOW_STRENGTH))]
    #[serde(alias = "glow_strength")]
    pub glow_strength: f32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_GLOW_EXPOSURE))]
    #[serde(alias = "glow_exposure")]
    pub glow_exposure: f32,

    #[cfg_attr(feature = "cli", arg(long, value_parser = parse_off_light_color, default_value = "64,64,64"))]
    #[serde(alias = "off_light_color")]
    pub off_light_color: [u8; 3],

    #[cfg_attr(feature = "cli", arg(long, value_parser = parse_off_light_color, default_value = "16,16,16"))]
    #[serde(alias = "canvas_background")]
    pub canvas_background: [u8; 3],
}

impl Default for LedConfig {
    fn default() -> Self {
        LedConfig {
            border: DEFAULT_BORDER,
            led_size: DEFAULT_LED_SIZE,
            led_gap: DEFAULT_LED_GAP,
            led_shape: DEFAULT_LED_SHAPE,
            led_exposure: DEFAULT_LED_EXPOSURE,
            enable_glow: DEFAULT_ENABLE_GLOW,
            glow_range: DEFAULT_GLOW_RANGE,
            glow_strength: DEFAULT_GLOW_STRENGTH,
            glow_exposure: DEFAULT_GLOW_EXPOSURE,
            off_light_color: DEFAULT_OFF_LIGHT_COLOR,
            canvas_background: DEFAULT_CANVAS_BACKGROUND,
        }
    }
}

#[derive(Debug, Error)]
pub enum LedError {
    #[error("failed to decode image: {0}")]
    FailedDecode(String),

    #[error("invalid config: {0}")]
    InvalidConfiguration(String),

    #[error("image processing failed: {0}")]
    FailedImageProcessing(String),

    #[error("failed to encode image: {0}")]
    FailedEncode(String),
}

pub fn generate_led_image(
    original_img: image::RgbImage,
    led_config: &LedConfig,
) -> Result<image::RgbImage, LedError> {
    LedPipeline::new(led_config).generate(original_img)
}

/// Per-configuration data shared across frames.
///
/// `Lut`s and the LED `stamp` depend only on the config, so video pipelines
/// build this once and reuse it for every frame instead of recomputing them
/// per frame.
#[derive(Clone)]
pub struct LedPipeline {
    config: LedConfig,
    base_lut: Lut,
    glow_lut: Lut,
    stamp: Vec<f32>,
}

impl LedPipeline {
    pub fn new(led_config: &LedConfig) -> Self {
        Self {
            config: led_config.clone(),
            base_lut: Lut::new(led_config.led_exposure),
            glow_lut: Lut::new(led_config.glow_exposure),
            stamp: create_stamp(led_config.led_size, &led_config.led_shape),
        }
    }

    pub fn config(&self) -> &LedConfig {
        &self.config
    }

    /// Output canvas dimensions for a given input size.
    pub fn output_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        let step = self.config.led_size + self.config.led_gap;
        let content_width = (width * step).saturating_sub(self.config.led_gap);
        let content_height = (height * step).saturating_sub(self.config.led_gap);
        (
            content_width + self.config.border * 2,
            content_height + self.config.border * 2,
        )
    }

    /// Single frame conversion, using inner (row-level) parallelism.
    pub fn generate(&self, original_img: image::RgbImage) -> Result<image::RgbImage, LedError> {
        self.render(original_img, true)
    }

    /// Single frame conversion without inner parallelism.
    ///
    /// Used when frames themselves are processed in parallel
    /// ([`LedPipeline::process_batch`]) to avoid nested-par oversubscription.
    pub fn generate_serial(
        &self,
        original_img: image::RgbImage,
    ) -> Result<image::RgbImage, LedError> {
        self.render(original_img, false)
    }

    /// Convert multiple frames, preserving input order.
    ///
    /// Frames are processed in parallel with rayon (sequentially on wasm32).
    /// Each frame uses the serial inner path so one frame occupies ~one worker.
    pub fn process_batch(
        &self,
        frames: Vec<image::RgbImage>,
    ) -> Result<Vec<image::RgbImage>, LedError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            frames
                .into_par_iter()
                .map(|frame| self.generate_serial(frame))
                .collect()
        }
        #[cfg(target_arch = "wasm32")]
        {
            frames
                .into_iter()
                .map(|frame| self.generate_serial(frame))
                .collect()
        }
    }

    fn render(
        &self,
        original_img: image::RgbImage,
        parallel: bool,
    ) -> Result<image::RgbImage, LedError> {
        let led_config = &self.config;
        let base_lut = &self.base_lut;
        let glow_lut = &self.glow_lut;
        let stamp = &self.stamp;

        let (width, height) = original_img.dimensions();

        let step = led_config.led_size + led_config.led_gap;
        let content_width = (width * step).saturating_sub(led_config.led_gap);
        let content_height = (height * step).saturating_sub(led_config.led_gap);
        let canvas_width = content_width + led_config.border * 2;
        let canvas_height = content_height + led_config.border * 2;

        let mut base_canvas = image::RgbImage::from_pixel(
            canvas_width,
            canvas_height,
            image::Rgb(led_config.canvas_background),
        );
        let mut glow_canvas = image::RgbImage::new(canvas_width, canvas_height);
        let bytes_per_row = canvas_width as usize * 3;

        let base_raw = base_canvas.as_mut();
        let glow_raw = glow_canvas.as_mut();

        let top_offset = led_config.border as usize * bytes_per_row;
        let content_len = content_height as usize * bytes_per_row;
        let base_content = &mut base_raw[top_offset..top_offset + content_len];
        let glow_content = &mut glow_raw[top_offset..top_offset + content_len];

        let process_row = |content_y: usize, base_row: &mut [u8], glow_row: &mut [u8]| {
            let content_y = content_y as u32;
            let original_y = content_y / step;
            let sy = content_y % step;

            if sy >= led_config.led_size {
                return;
            }

            for original_x in 0..width {
                let rgb = original_img.get_pixel(original_x, original_y).0;

                let base_color = base_lut.apply(&rgb);
                let glow_color = glow_lut.apply(&rgb);

                let target_base = [
                    (base_color[0] as f32).max(led_config.off_light_color[0] as f32),
                    (base_color[1] as f32).max(led_config.off_light_color[1] as f32),
                    (base_color[2] as f32).max(led_config.off_light_color[2] as f32),
                ];

                let target_glow = [
                    glow_color[0] as f32,
                    glow_color[1] as f32,
                    glow_color[2] as f32,
                ];

                let canvas_base_x = original_x * step;

                for sx in 0..led_config.led_size {
                    let alpha = stamp[(sy * led_config.led_size + sx) as usize];

                    if alpha > 0.0 {
                        let pixel_pos = (led_config.border + canvas_base_x + sx) as usize * 3;

                        let bg = led_config.canvas_background;

                        base_row[pixel_pos] =
                            ((target_base[0] * alpha) + (bg[0] as f32 * (1.0 - alpha))) as u8;
                        base_row[pixel_pos + 1] =
                            ((target_base[1] * alpha) + (bg[1] as f32 * (1.0 - alpha))) as u8;
                        base_row[pixel_pos + 2] =
                            ((target_base[2] * alpha) + (bg[2] as f32 * (1.0 - alpha))) as u8;

                        glow_row[pixel_pos] = (target_glow[0] * alpha) as u8;
                        glow_row[pixel_pos + 1] = (target_glow[1] * alpha) as u8;
                        glow_row[pixel_pos + 2] = (target_glow[2] * alpha) as u8;
                    }
                }
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        if parallel {
            base_content
                .par_chunks_mut(bytes_per_row)
                .zip(glow_content.par_chunks_mut(bytes_per_row))
                .enumerate()
                .for_each(|(content_y, (base_row, glow_row))| {
                    process_row(content_y, base_row, glow_row);
                });
        } else {
            for (content_y, (base_row, glow_row)) in base_content
                .chunks_mut(bytes_per_row)
                .zip(glow_content.chunks_mut(bytes_per_row))
                .enumerate()
            {
                process_row(content_y, base_row, glow_row);
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = parallel;
            for (content_y, (base_row, glow_row)) in base_content
                .chunks_mut(bytes_per_row)
                .zip(glow_content.chunks_mut(bytes_per_row))
                .enumerate()
            {
                process_row(content_y, base_row, glow_row);
            }
        }

        if led_config.enable_glow && led_config.glow_range > 0.0 {
            let glow_blurred = self.blur_glow(&glow_canvas);

            let strength = led_config.glow_strength;
            let glow_blurred_raw = glow_blurred.as_raw();

            let blend_pixel = |base_pixel: &mut [u8], glow_pixel: &[u8]| {
                for i in 0..3 {
                    let glow_val = (glow_pixel[i] as f32 * strength).clamp(0.0, 255.0);
                    let base_val = base_pixel[i] as f32;

                    // スクリーン合成公式: 255 - ((255 - Base) * (255 - Glow) / 255)
                    base_pixel[i] =
                        (255.0 - ((255.0 - base_val) * (255.0 - glow_val) / 255.0)) as u8;
                }
            };

            #[cfg(not(target_arch = "wasm32"))]
            if parallel {
                base_canvas
                    .as_mut()
                    .par_chunks_mut(3)
                    .zip(glow_blurred_raw.par_chunks(3))
                    .for_each(|(base_pixel, glow_pixel)| {
                        blend_pixel(base_pixel, glow_pixel);
                    });
            } else {
                for (base_pixel, glow_pixel) in base_canvas
                    .as_mut()
                    .chunks_mut(3)
                    .zip(glow_blurred_raw.chunks(3))
                {
                    blend_pixel(base_pixel, glow_pixel);
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                let _ = parallel;
                for (base_pixel, glow_pixel) in base_canvas
                    .as_mut()
                    .chunks_mut(3)
                    .zip(glow_blurred_raw.chunks(3))
                {
                    blend_pixel(base_pixel, glow_pixel);
                }
            }
        }
        Ok(base_canvas)
    }

    fn blur_glow(&self, glow_canvas: &image::RgbImage) -> image::RgbImage {
        #[cfg(not(target_arch = "wasm32"))]
        {
            blur_rgb_parallel(glow_canvas, self.config.glow_range)
        }
        #[cfg(target_arch = "wasm32")]
        {
            image::imageops::blur(glow_canvas, self.config.glow_range)
        }
    }
}

/// Radius (in pixels) of the 1-D gaussian kernel that
/// `image::imageops::blur` builds for `sigma`.
///
/// Mirrors `GaussianBlurParameters::kernel_size_from_sigma` in image 0.25.
fn gaussian_kernel_radius(sigma: f32) -> u32 {
    let possible_size = ((((sigma - 0.8) / 0.3) + 1.0) * 2.0 + 1.0).max(3.0) as u32;
    let kernel_size = if possible_size.is_multiple_of(2) {
        possible_size + 1
    } else {
        possible_size
    };
    kernel_size / 2
}

/// Parallel gaussian blur with output identical to `image::imageops::blur`.
///
/// The image is split into full-width horizontal strips, each carrying
/// `halo` rows of overlap top/bottom. Every output pixel is therefore
/// computed from exactly the same input neighbourhood (clamped edges behave
/// identically) as a whole-image blur, so results are bit-identical while
/// strips blur concurrently on the rayon pool.
#[cfg(not(target_arch = "wasm32"))]
fn blur_rgb_parallel(image: &image::RgbImage, sigma: f32) -> image::RgbImage {
    let (width, height) = image.dimensions();
    let strip_count = (rayon::current_num_threads().max(1) as u32)
        .min(height.max(1))
        .max(1);
    if strip_count <= 1 {
        return image::imageops::blur(image, sigma);
    }
    let halo = gaussian_kernel_radius(sigma);
    let base_rows = height / strip_count;
    let extra = height % strip_count;

    let mut bounds = Vec::with_capacity(strip_count as usize);
    let mut y = 0u32;
    for i in 0..strip_count {
        let rows = base_rows + u32::from(i < extra);
        bounds.push((y, rows));
        y += rows;
    }

    let bytes_per_row = width as usize * 3;
    let strips: Vec<Vec<u8>> = bounds
        .par_iter()
        .map(|&(out_y, rows)| {
            let top = out_y.saturating_sub(halo);
            let bottom = (out_y + rows + halo).min(height);
            let strip = image::imageops::crop_imm(image, 0, top, width, bottom - top).to_image();
            let blurred = image::imageops::blur(&strip, sigma);
            let raw = blurred.into_raw();
            let skip = (out_y - top) as usize * bytes_per_row;
            let take = rows as usize * bytes_per_row;
            raw[skip..skip + take].to_vec()
        })
        .collect();

    let mut out = Vec::with_capacity(width as usize * height as usize * 3);
    for strip in &strips {
        out.extend_from_slice(strip);
    }
    image::RgbImage::from_raw(width, height, out).expect("tiled blur output size must match")
}

pub fn create_stamp(led_size: u32, shape: &LedShape) -> Vec<f32> {
    let mut pixmap =
        Pixmap::new(led_size, led_size).expect("Failed to create a pixel map for the stamp");

    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255); // 白（不透明）
    paint.anti_alias = true;

    let size_f32 = led_size as f32;

    let path = match shape {
        LedShape::Circle => {
            let radius = size_f32 / 2.0;
            PathBuilder::from_circle(radius, radius, radius)
                .expect("Failed to generate a circular path")
        }
        LedShape::Square => {
            let rect = Rect::from_xywh(0.0, 0.0, size_f32, size_f32)
                .expect("Failed to generate a rectangle");
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            pb.finish().expect("Failed to generate a rectangular path")
        }
    };

    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    pixmap
        .pixels()
        .iter()
        .map(|pixel| pixel.alpha() as f32 / 255.0)
        .collect()
}

#[derive(Clone)]
struct Lut {
    data: [u8; 256],
}

impl Lut {
    pub fn new(exposure: f32) -> Self {
        let mut data = [0u8; 256];
        for i in 0..=255 {
            data[i] = (i as f32 * exposure).clamp(0.0, 255.0) as u8;
        }
        Self { data }
    }

    pub fn apply(&self, color: &[u8; 3]) -> [u8; 3] {
        color.map(|c| self.data[c as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient_image(width: u32, height: u32, seed: u8) -> image::RgbImage {
        let mut img = image::RgbImage::new(width, height);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgb([
                x.wrapping_add(y).wrapping_add(seed as u32) as u8,
                (x * 3 + y * 5 + seed as u32) as u8,
                (x * 7 + y * 11 + seed as u32 * 13) as u8,
            ]);
        }
        img
    }

    fn glow_config() -> LedConfig {
        LedConfig {
            border: 2,
            led_size: 4,
            led_gap: 2,
            led_shape: LedShape::Circle,
            led_exposure: 1.2,
            enable_glow: true,
            glow_range: 3.0,
            glow_strength: 1.75,
            glow_exposure: 1.0,
            off_light_color: [64, 64, 64],
            canvas_background: [16, 16, 16],
        }
    }

    fn flat_config() -> LedConfig {
        LedConfig {
            enable_glow: false,
            led_shape: LedShape::Square,
            ..glow_config()
        }
    }

    #[test]
    fn pipeline_matches_free_function() {
        for config in [glow_config(), flat_config()] {
            let pipeline = LedPipeline::new(&config);
            for seed in [0u8, 1, 42] {
                let img = gradient_image(17, 11, seed);
                let expected = generate_led_image(img.clone(), &config).unwrap();
                assert_eq!(
                    pipeline.generate(img.clone()).unwrap().as_raw(),
                    expected.as_raw()
                );
                assert_eq!(
                    pipeline.generate_serial(img.clone()).unwrap().as_raw(),
                    expected.as_raw()
                );
            }
        }
    }

    #[test]
    fn batch_preserves_order_and_bytes() {
        let config = glow_config();
        let pipeline = LedPipeline::new(&config);
        let frames: Vec<image::RgbImage> = (0..9)
            .map(|seed| gradient_image(13 + seed, 7 + seed, seed as u8))
            .collect();
        let expected: Vec<Vec<u8>> = frames
            .iter()
            .map(|f| generate_led_image(f.clone(), &config).unwrap().into_raw())
            .collect();
        let batched = pipeline.process_batch(frames).unwrap();
        assert_eq!(batched.len(), expected.len());
        for (got, want) in batched.iter().zip(expected.iter()) {
            assert_eq!(got.as_raw(), want);
        }
    }

    #[test]
    fn kernel_radius_matches_image_crate() {
        // image 0.25: ((((sigma - 0.8) / 0.3) + 1) * 2 + 1), at least 3, rounded up to odd.
        assert_eq!(gaussian_kernel_radius(3.0), 8);
        assert_eq!(gaussian_kernel_radius(0.8), 1);
        assert_eq!(gaussian_kernel_radius(1.1), 2);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn tiled_blur_matches_scalar_blur() {
        // Sizes chosen to hit strip boundaries, remainders and tiny images.
        for (w, h) in [(3, 2), (5, 1), (16, 7), (64, 13), (96, 48)] {
            let img = gradient_image(w, h, 9);
            for sigma in [0.5, 1.0, 3.0, 5.5] {
                let expected = image::imageops::blur(&img, sigma);
                let got = blur_rgb_parallel(&img, sigma);
                assert_eq!(
                    got.as_raw(),
                    expected.as_raw(),
                    "tiled blur mismatch at {w}x{h} sigma={sigma}"
                );
            }
        }
    }
}
