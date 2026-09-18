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

        // Per-source-pixel colors, computed once per frame. The row loop
        // below visits each source row `led_size` times (once per canvas
        // row), so folding the LUT + off-light max in here removes that
        // redundant work. u8 max + later exact u8->f32 cast keeps results
        // bit-identical to the inline computation.
        let input_raw = original_img.as_raw();
        let pixel_count = width as usize * height as usize;
        let mut base_table = vec![0u8; pixel_count * 3];
        let mut glow_table = vec![0u8; pixel_count * 3];
        for (i, px) in input_raw.chunks_exact(3).enumerate() {
            let rgb = [px[0], px[1], px[2]];
            let base_color = base_lut.apply(&rgb);
            let glow_color = glow_lut.apply(&rgb);
            let o = i * 3;
            base_table[o] = base_color[0].max(led_config.off_light_color[0]);
            base_table[o + 1] = base_color[1].max(led_config.off_light_color[1]);
            base_table[o + 2] = base_color[2].max(led_config.off_light_color[2]);
            glow_table[o] = glow_color[0];
            glow_table[o + 1] = glow_color[1];
            glow_table[o + 2] = glow_color[2];
        }

        let process_row = |content_y: usize, base_row: &mut [u8], glow_row: &mut [u8]| {
            let content_y = content_y as u32;
            let original_y = content_y / step;
            let sy = content_y % step;

            if sy >= led_config.led_size {
                return;
            }

            let table_row = original_y as usize * width as usize * 3;
            for original_x in 0..width {
                let t = table_row + original_x as usize * 3;

                let target_base = [
                    base_table[t] as f32,
                    base_table[t + 1] as f32,
                    base_table[t + 2] as f32,
                ];

                let target_glow = [
                    glow_table[t] as f32,
                    glow_table[t + 1] as f32,
                    glow_table[t + 2] as f32,
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
            let glow_blurred = self.blur_glow(&glow_canvas, parallel)?;

            let strength = led_config.glow_strength;
            let glow_blurred_raw = glow_blurred.as_raw();
            // Screen blend with black glow is the identity (screen(b, 0)
            // == b: the float chain collapses to exactly `b`), so gap and
            // border pixels can skip the math. Only valid for finite
            // strength, since 0 * NaN/inf is NaN rather than 0.
            let finite_strength = strength.is_finite();

            let blend_pixel = |base_pixel: &mut [u8], glow_pixel: &[u8]| {
                if finite_strength
                    && glow_pixel[0] == 0
                    && glow_pixel[1] == 0
                    && glow_pixel[2] == 0
                {
                    return;
                }
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

    fn blur_glow(
        &self,
        glow_canvas: &image::RgbImage,
        parallel: bool,
    ) -> Result<image::RgbImage, LedError> {
        // Same sigma and kernel size the image crate derives from
        // `glow_range`, but executed by libblur's SIMD separable gaussian
        // (analytical kernel, clamped edges) instead of the scalar one.
        // `FixedPoint` keeps ~1-3% error at ~2x the speed of `Exact`.
        let (width, height) = glow_canvas.dimensions();
        let kernel = gaussian_kernel_radius(self.config.glow_range).max(1) * 2 + 1;
        let src = libblur::BlurImage::borrow(
            glow_canvas.as_raw(),
            width,
            height,
            libblur::FastBlurChannels::Channels3,
        );
        let mut dst_buf = vec![0u8; glow_canvas.as_raw().len()];
        {
            let mut dst = libblur::BlurImageMut::borrow(
                &mut dst_buf,
                width,
                height,
                libblur::FastBlurChannels::Channels3,
            );
            libblur::gaussian_blur(
                &src,
                &mut dst,
                libblur::GaussianBlurParams::new(kernel, self.config.glow_range as f64),
                libblur::EdgeMode2D::new(libblur::EdgeMode::Clamp),
                blur_threading_policy(parallel),
                libblur::ConvolutionMode::FixedPoint,
            )
            .map_err(|e| LedError::FailedImageProcessing(format!("glow blur failed: {e}")))?;
        }
        image::RgbImage::from_raw(width, height, dst_buf).ok_or_else(|| {
            LedError::FailedImageProcessing("glow blur output size mismatch".to_string())
        })
    }
}

/// Threading policy for the glow blur.
///
/// Single-frame `generate` runs its own rayon row pass first and the blur
/// afterwards, so the blur may use the pool (`Adaptive`). Inside
/// `process_batch` each frame already occupies a worker thread, so the blur
/// must stay single-threaded to avoid oversubscription. wasm32 is always
/// single-threaded.
#[cfg(not(target_arch = "wasm32"))]
fn blur_threading_policy(parallel: bool) -> libblur::ThreadingPolicy {
    if parallel {
        libblur::ThreadingPolicy::Adaptive
    } else {
        libblur::ThreadingPolicy::Single
    }
}

#[cfg(target_arch = "wasm32")]
fn blur_threading_policy(_parallel: bool) -> libblur::ThreadingPolicy {
    libblur::ThreadingPolicy::Single
}

/// Radius (in pixels) of the 1-D gaussian kernel that
/// `image::imageops::blur` builds for `sigma`.
///
/// Mirrors `GaussianBlurParameters::kernel_size_from_sigma` in image 0.25.
/// Kept as the user-facing `glow_range` (sigma) to libblur-radius mapping so
/// the setting keeps its meaning after the backend swap.
fn gaussian_kernel_radius(sigma: f32) -> u32 {
    let possible_size = ((((sigma - 0.8) / 0.3) + 1.0) * 2.0 + 1.0).max(3.0) as u32;
    let kernel_size = if possible_size.is_multiple_of(2) {
        possible_size + 1
    } else {
        possible_size
    };
    kernel_size / 2
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

    #[test]
    fn blur_glow_properties() {
        let config = glow_config();
        let pipeline = LedPipeline::new(&config);
        let (cw, ch) = pipeline.output_dimensions(16, 7);

        // Black stays black through the blur.
        let black = image::RgbImage::new(cw, ch);
        let blurred = pipeline.blur_glow(&black, false).unwrap();
        assert_eq!(blurred.dimensions(), (cw, ch));
        assert!(blurred.as_raw().iter().all(|&b| b == 0));

        // Dimensions are preserved across shapes, incl. tiny images, and
        // the blur is deterministic.
        for (w, h) in [(1, 1), (3, 2), (5, 1), (16, 7), (64, 13)] {
            let canvas = gradient_image(w, h, 9);
            let a = pipeline.blur_glow(&canvas, false).unwrap();
            let b = pipeline.blur_glow(&canvas, false).unwrap();
            assert_eq!(a.dimensions(), (w, h));
            assert_eq!(a.as_raw(), b.as_raw());
        }

        // A single bright dot spreads light into its neighbourhood while
        // distant pixels stay dark.
        let mut dot = image::RgbImage::new(64, 64);
        dot.put_pixel(32, 32, image::Rgb([255, 255, 255]));
        let blurred = pipeline.blur_glow(&dot, false).unwrap();
        let center = blurred.get_pixel(32, 32).0;
        assert!(center[0] > 0, "blur must keep light at the source");
        let far = blurred.get_pixel(0, 0).0;
        assert_eq!(far, [0, 0, 0], "blur must not leak across the image");
        let near: u32 = blurred.get_pixel(33, 32).0.iter().map(|&b| b as u32).sum();
        assert!(near > 0, "blur must spread light to neighbours");
    }

    #[test]
    fn blur_glow_stays_close_to_reference_blur() {
        // Locks in the visual-equivalence property of the libblur backend:
        // same sigma/kernel/edges as `image::imageops::blur` must keep
        // per-channel drift within ±2.
        let config = glow_config();
        let pipeline = LedPipeline::new(&config);
        for (w, h) in [(16, 7), (64, 13)] {
            let img = gradient_image(w, h, 9);
            let got = pipeline.blur_glow(&img, false).unwrap();
            let expected = image::imageops::blur(&img, config.glow_range);
            assert_eq!(got.dimensions(), expected.dimensions());
            let max_diff = got
                .as_raw()
                .iter()
                .zip(expected.as_raw().iter())
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            assert!(
                max_diff <= 2,
                "glow blur drifted too far from reference at {w}x{h}: max_abs_diff={max_diff}"
            );
        }
    }

    #[test]
    fn blur_glow_parallel_matches_serial_shape() {
        // Parallel/single policies may differ by rounding, but geometry and
        // brightness conservation must hold for both.
        let config = glow_config();
        let pipeline = LedPipeline::new(&config);
        let img = gradient_image(48, 32, 7);
        for parallel in [false, true] {
            let out = pipeline.blur_glow(&img, parallel).unwrap();
            assert_eq!(out.dimensions(), img.dimensions());
            let sum_in: u64 = img.as_raw().iter().map(|&b| b as u64).sum();
            let sum_out: u64 = out.as_raw().iter().map(|&b| b as u64).sum();
            let drift = sum_in.abs_diff(sum_out) as f64 / sum_in.max(1) as f64;
            assert!(
                drift < 0.05,
                "blur must roughly conserve brightness (drift={drift})"
            );
        }
    }
}
