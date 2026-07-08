mod utils;

use wasm_bindgen::prelude::*;

use led_gen_core::{generate_led_image, LedConfig};

use js_sys::Uint8Array;

#[wasm_bindgen]
pub struct ProcessedImageResult {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[wasm_bindgen]
impl ProcessedImageResult {
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> Uint8Array {
        Uint8Array::from(self.data.as_slice())
    }
}

#[wasm_bindgen]
pub fn generate_led_image_wasm(
    image_data: &[u8],
    width: u32,
    height: u32,
    config: LedConfig
) -> Result<ProcessedImageResult, JsValue> {
    utils::set_panic_hook();

    let mut raw_rgb = Vec::with_capacity((width * height * 3) as usize);

    for chunk in image_data.chunks_exact(4) {
        raw_rgb.push(chunk[0]);
        raw_rgb.push(chunk[1]);
        raw_rgb.push(chunk[2]);
    }

    let original_img = image::RgbImage::from_raw(width, height, raw_rgb)
        .ok_or_else(|| JsValue::from_str("failed to create ImageBuffer"))?;


    let processed_img = generate_led_image(original_img, &config)
        .map_err(|e| JsValue::from_str(&format!("failed to generate LED image: {e}")))?;

    let (out_width, out_height) = processed_img.dimensions();
    let mut out_rgba = Vec::with_capacity((out_width * out_height * 4) as usize);

    for pixel in processed_img.pixels() {
        out_rgba.push(pixel[0]);
        out_rgba.push(pixel[1]);
        out_rgba.push(pixel[2]);
        out_rgba.push(255);
    };

    Ok(ProcessedImageResult{
        width: out_width,
        height: out_height,
        data: out_rgba
    })
}