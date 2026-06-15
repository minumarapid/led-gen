#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};
use serde::Deserialize;
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
#[tsify(from_wasm_abi)]
pub enum LedShape {
    Square,
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
    pub led_size: u32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_LED_GAP))]

    pub led_gap: u32,

    #[cfg_attr(feature = "cli", arg(long, value_enum, default_value_t = DEFAULT_LED_SHAPE))]
    pub led_shape: LedShape,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_LED_EXPOSURE))]
    pub led_exposure: f32,

    #[cfg_attr(feature = "cli", arg(long = "no-glow", action = clap::ArgAction::SetFalse))]
    pub enable_glow: bool,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_GLOW_RANGE))]
    pub glow_range: f32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_GLOW_STRENGTH))]
    pub glow_strength: f32,

    #[cfg_attr(feature = "cli", arg(long, default_value_t = DEFAULT_GLOW_EXPOSURE))]
    pub glow_exposure: f32,

    #[cfg_attr(feature = "cli", arg(long, value_parser = parse_off_light_color, default_value = "64,64,64"))]
    pub off_light_color: [u8; 3],

    #[cfg_attr(feature = "cli", arg(long, value_parser = parse_off_light_color, default_value = "16,16,16"))]
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

pub enum LedError {
    FailedDecode(String),
    InvalidConfiguration(String),
    FailedEncode(String)
}

pub fn generate_led_image(original_img:image::RgbImage, led_config: &LedConfig ) -> Result<image::RgbImage,LedError> {

    let base_lut = Lut::new(led_config.led_exposure);
    let glow_lut = Lut::new(led_config.glow_exposure);

    let stamp = create_stamp(led_config.led_size, &led_config.led_shape);

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
                glow_color[0] as f32, glow_color[1] as f32, glow_color[2] as f32,
            ];

            let canvas_base_x = original_x * step;

            for sx in 0..led_config.led_size {
                let alpha = stamp[(sy * led_config.led_size + sx) as usize];

                if alpha > 0.0 {
                    let pixel_pos = (led_config.border + canvas_base_x + sx) as usize * 3;

                    let bg = led_config.canvas_background;

                    base_row[pixel_pos]     = ((target_base[0] * alpha) + (bg[0] as f32 * (1.0 - alpha))) as u8;
                    base_row[pixel_pos + 1] = ((target_base[1] * alpha) + (bg[1] as f32 * (1.0 - alpha))) as u8;
                    base_row[pixel_pos + 2] = ((target_base[2] * alpha) + (bg[2] as f32 * (1.0 - alpha))) as u8;

                    glow_row[pixel_pos]     = (target_glow[0] * alpha) as u8;
                    glow_row[pixel_pos + 1] = (target_glow[1] * alpha) as u8;
                    glow_row[pixel_pos + 2] = (target_glow[2] * alpha) as u8;
                }
            }
        }
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        base_content
            .par_chunks_mut(bytes_per_row)
            .zip(glow_content.par_chunks_mut(bytes_per_row))
            .enumerate()
            .for_each(|(content_y, (base_row, glow_row))| {
                process_row(content_y, base_row, glow_row);
            });
    }

    #[cfg(target_arch = "wasm32")]
    {
        for (content_y, (base_row, glow_row)) in base_content
            .chunks_mut(bytes_per_row)
            .zip(glow_content.chunks_mut(bytes_per_row))
            .enumerate()
        {
            process_row(content_y, base_row, glow_row);
        }
    }

    if led_config.enable_glow && led_config.glow_range > 0.0 {
        let glow_blurred = image::imageops::blur(&glow_canvas, led_config.glow_range);

        let strength = led_config.glow_strength;
        let glow_blurred_raw = glow_blurred.as_raw();


        let blend_pixel = |base_pixel: &mut [u8], glow_pixel: &[u8]| {
            for i in 0..3 {
                let glow_val = (glow_pixel[i] as f32 * strength).clamp(0.0, 255.0);
                let base_val = base_pixel[i] as f32;

                // スクリーン合成公式: 255 - ((255 - Base) * (255 - Glow) / 255)
                base_pixel[i] = (255.0 - ((255.0 - base_val) * (255.0 - glow_val) / 255.0)) as u8;
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            base_canvas
                .as_mut()
                .par_chunks_mut(3)
                .zip(glow_blurred_raw.par_chunks(3))
                .for_each(|(base_pixel, glow_pixel)| {
                    blend_pixel(base_pixel, glow_pixel);
                });
        }

        #[cfg(target_arch = "wasm32")]
        {
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

pub fn create_stamp(led_size: u32, shape: &LedShape) -> Vec<f32> {
    let mut pixmap = Pixmap::new(led_size, led_size)
        .expect("Failed to create a pixel map for the stamp");

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
