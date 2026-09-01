use std::{collections::HashMap, sync::OnceLock};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ImageEncoder, Rgba, RgbaImage, codecs::png::PngEncoder, imageops::FilterType};
use tokio::sync::RwLock;

fn bytes_cache() -> &'static RwLock<HashMap<String, Vec<u8>>> {
	static CACHE: OnceLock<RwLock<HashMap<String, Vec<u8>>>> = OnceLock::new();
	CACHE.get_or_init(Default::default)
}

// Downloads (or reuses a cached copy of) the raw bytes at `url`.
pub async fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
	if let Some(cached) = bytes_cache().read().await.get(url) {
		return Some(cached.clone());
	}

	let bytes = match reqwest::Client::new().get(url).send().await {
		Ok(response) => match response.bytes().await {
			Ok(bytes) => bytes.to_vec(),
			Err(e) => {
				log::error!("Failed to read bytes from '{}': {}", url, e);
				return None;
			}
		},
		Err(e) => {
			log::error!("Failed to download '{}': {}", url, e);
			return None;
		}
	};

	bytes_cache()
		.write()
		.await
		.insert(url.to_owned(), bytes.clone());

	Some(bytes)
}

// Decodes an image and resizes it to `size`x`size`.
fn decode_and_resize(bytes: &[u8], size: u32) -> Option<RgbaImage> {
	let source = match image::load_from_memory(bytes) {
		Ok(image) => image.to_rgba8(),
		Err(e) => {
			log::error!("Failed to decode image ({} bytes): {}", bytes.len(), e);
			return None;
		}
	};
	Some(image::imageops::resize(
		&source,
		size,
		size,
		FilterType::Lanczos3,
	))
}

fn encode_png(image: &RgbaImage, size: u32) -> Option<Vec<u8>> {
	let mut png_bytes = Vec::new();
	if let Err(e) = PngEncoder::new(&mut png_bytes).write_image(
		image.as_raw(),
		size,
		size,
		image::ExtendedColorType::Rgba8,
	) {
		log::error!("Failed to encode image as PNG: {}", e);
		return None;
	}
	Some(png_bytes)
}

// Decodes an image and resizes it to `size`x`size`, without cropping or any other processing,
// and re-encodes it as PNG bytes.
pub fn resized_png(bytes: &[u8], size: u32) -> Option<Vec<u8>> {
	let image = decode_and_resize(bytes, size)?;
	encode_png(&image, size)
}

// Decodes an image, crops it into a `size`x`size` circle (transparent outside it), optionally
// drawing a colored ring (`border`: width in px, color) just inside the edge, and re-encodes
// the result as PNG bytes.
pub fn circular_png(bytes: &[u8], size: u32, border: Option<(f32, Rgba<u8>)>) -> Option<Vec<u8>> {
	let source = decode_and_resize(bytes, size)?;

	let center = size as f32 / 2.0;
	let inner_radius = match border {
		Some((width, _)) => center - width,
		None => center,
	};

	let mut out = RgbaImage::new(size, size);
	for y in 0..size {
		for x in 0..size {
			let dx = x as f32 + 0.5 - center;
			let dy = y as f32 + 0.5 - center;
			let dist = dx.hypot(dy);

			let pixel = if dist > center {
				Rgba([0, 0, 0, 0])
			} else if let Some((_, color)) = border
				&& dist > inner_radius
			{
				color
			} else {
				*source.get_pixel(x, y)
			};
			out.put_pixel(x, y, pixel);
		}
	}

	encode_png(&out, size)
}

pub fn to_data_uri(png_bytes: &[u8]) -> String {
	format!("data:image/png;base64,{}", STANDARD.encode(png_bytes))
}
