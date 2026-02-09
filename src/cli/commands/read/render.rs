use {
    arboard::{Clipboard, ImageData},
    boundbook::{BbfError, BbfReader, Result},
    clap::ValueEnum,
    gif::DecodeOptions,
    gif_dispose::Screen as GifScreen,
    image::{DynamicImage, ImageReader, RgbaImage, imageops::FilterType},
    miette::{Context, IntoDiagnostic},
    rayon::iter::{IntoParallelIterator, ParallelIterator},
    std::io::Cursor,
};

use super::interpolate::{FrameInterpolator, InterpolationMethod};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ScalingFilter {
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

impl From<ScalingFilter> for FilterType {
    fn from(filter: ScalingFilter) -> Self {
        match filter {
            ScalingFilter::Nearest => FilterType::Nearest,
            ScalingFilter::Triangle => FilterType::Triangle,
            ScalingFilter::CatmullRom => FilterType::CatmullRom,
            ScalingFilter::Gaussian => FilterType::Gaussian,
            ScalingFilter::Lanczos3 => FilterType::Lanczos3,
        }
    }
}

#[derive(Clone)]
pub struct RenderConfig {
    pub enable_gif_animation: bool,
    pub gif_speed: f32,
    pub gif_loop: bool,
    pub gif_interpolate: usize,
    pub interpolation_method: InterpolationMethod,
}

pub struct ImageRenderer {
    pub config: RenderConfig,
}

impl ImageRenderer {
    pub const fn new(config: RenderConfig) -> Self {
        Self { config }
    }

    pub fn is_gif(data: &[u8]) -> bool {
        data.len() > 3 && &data[0..3] == b"GIF"
    }

    /// Decode arbitrary image bytes into a `DynamicImage`.
    pub fn decode_image(data: &[u8]) -> Result<DynamicImage> {
        let img = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .into_diagnostic()
            .map_err(|e| format!("Failed to guess image format: {}", e))?
            .decode()
            .into_diagnostic()
            .map_err(|e| format!("Failed to decode image: {}", e))?;
        Ok(img)
    }

    pub fn decode_gif_first_frame(data: &[u8]) -> Result<DynamicImage> {
        let mut decode_options = DecodeOptions::new();
        decode_options.set_color_output(gif::ColorOutput::Indexed);

        let cursor = Cursor::new(data);
        let mut decoder = decode_options
            .read_info(cursor)
            .into_diagnostic()
            .context("Failed to decode GIF metadata")?;

        let mut screen = GifScreen::new_decoder(&decoder);

        if let Some(frame) = decoder
            .read_next_frame()
            .into_diagnostic()
            .context("Failed to read first GIF frame")?
        {
            screen
                .blit_frame(frame)
                .into_diagnostic()
                .context("Failed to composite GIF frame")?;

            let pixels = screen.pixels_rgba();
            let (rgba_vec, width, height) = pixels.to_contiguous_buf();

            let rgba_data: Vec<u8> = rgba_vec
                .iter()
                .flat_map(|rgba| [rgba.r, rgba.g, rgba.b, rgba.a])
                .collect();

            let img_buf =
                RgbaImage::from_raw(width as u32, height as u32, rgba_data).ok_or_else(|| {
                    BbfError::Other {
                        message: "Failed to create RGBA image buffer from GIF frame".to_string(),
                    }
                })?;

            Ok(DynamicImage::ImageRgba8(img_buf))
        } else {
            Err(BbfError::Other {
                message: "GIF contains no frames".to_string(),
            })
        }
    }

    /// # Panics
    ///
    /// panics if it fails to create an rgba buffer for a frame
    #[allow(clippy::arithmetic_side_effects)]
    pub fn decode_gif_frames(&self, data: &[u8]) -> Result<Vec<(DynamicImage, u64)>> {
        let mut decode_options = DecodeOptions::new();
        decode_options.set_color_output(gif::ColorOutput::Indexed);

        let cursor = Cursor::new(data);
        let mut decoder = decode_options
            .read_info(cursor)
            .into_diagnostic()
            .context("Failed to decode GIF for animation")?;
        let mut screen = GifScreen::new_decoder(&decoder);
        let mut raw_frames: Vec<(Vec<u8>, u32, u32, u32)> = Vec::with_capacity(64);

        while let Some(frame) = decoder
            .read_next_frame()
            .into_diagnostic()
            .context("Failed to read GIF animation frame")?
        {
            let delay = (frame.delay as u32 * 10).max(10);

            screen
                .blit_frame(frame)
                .into_diagnostic()
                .context("Failed to composite GIF animation frame")?;

            let pixels = screen.pixels_rgba();
            let (rgba_vec, width, height) = pixels.to_contiguous_buf();

            let mut rgba_data = Vec::with_capacity(rgba_vec.len() * 4);
            for rgba in rgba_vec.iter() {
                rgba_data.push(rgba.r);
                rgba_data.push(rgba.g);
                rgba_data.push(rgba.b);
                rgba_data.push(rgba.a);
            }

            raw_frames.push((rgba_data, width as u32, height as u32, delay));
        }

        if raw_frames.is_empty() {
            return Err(BbfError::Other {
                message: "GIF animation contains no frames".to_string(),
            });
        }

        let interpolate_count = self.config.gif_interpolate;
        let gif_speed = self.config.gif_speed;
        let interpolator = FrameInterpolator::new(self.config.interpolation_method);

        let mut frames: Vec<(DynamicImage, u64)> =
            Vec::with_capacity(raw_frames.len() * (interpolate_count + 1));

        for i in 0..raw_frames.len() {
            let (ref rgba_data, width, height, delay) = raw_frames[i];
            let adjusted_delay = (delay as f32 * gif_speed) as u64;
            let frame_delay = if interpolate_count > 0 {
                adjusted_delay / (interpolate_count + 1) as u64
            } else {
                adjusted_delay
            };

            let img_buf = RgbaImage::from_raw(width, height, rgba_data.clone())
                .expect("Failed to create RGBA buffer for GIF frame");
            frames.push((DynamicImage::ImageRgba8(img_buf), frame_delay));

            if interpolate_count > 0 {
                let next_idx = (i + 1) % raw_frames.len();
                let (ref next_rgba, next_width, next_height, _) = raw_frames[next_idx];

                if width == next_width && height == next_height {
                    let interp_images: Vec<DynamicImage> = (1..=interpolate_count)
                        .into_par_iter()
                        .map(|interp_i| {
                            let t = interp_i as f32 / (interpolate_count + 1) as f32;
                            let interpolated_rgba = interpolator
                                .interpolate_frames(rgba_data, next_rgba, width, height, t);

                            let buf = RgbaImage::from_raw(width, height, interpolated_rgba)
                                .expect("Failed to create interpolated RGBA buffer");
                            DynamicImage::ImageRgba8(buf)
                        })
                        .collect();

                    for img in interp_images {
                        frames.push((img, frame_delay));
                    }
                }
            }
        }

        Ok(frames)
    }

    pub fn copy_image_to_clipboard(&self, reader: &BbfReader, current_page: usize) -> Result<()> {
        let pages = reader.pages().into_diagnostic()?;
        if current_page >= pages.len() {
            return Err(miette::miette!("Current page index out of bounds").into());
        }

        let page = &pages[current_page];
        let assets = reader.assets().into_diagnostic()?;
        let asset = &assets[page.asset_index as usize];
        let data = reader.get_asset_data(asset).into_diagnostic()?;

        let img = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .into_diagnostic()
            .context("Failed to guess image format")?
            .decode()
            .into_diagnostic()
            .context("Failed to decode image")?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let img_data = ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into_raw().into(),
        };

        let mut clipboard = Clipboard::new()
            .into_diagnostic()
            .context("Failed to access clipboard")?;

        clipboard
            .set_image(img_data)
            .into_diagnostic()
            .context("Failed to copy image to clipboard")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused, clippy::missing_panics_doc)]
    use {super::*, assert2::check as assert};

    fn default_config() -> RenderConfig {
        RenderConfig {
            enable_gif_animation: false,
            gif_speed: 1.0,
            gif_loop: false,
            gif_interpolate: 0,
            interpolation_method: InterpolationMethod::Blend,
        }
    }

    #[test]
    fn test_is_gif_with_gif_magic() {
        assert!(ImageRenderer::is_gif(b"GIF89a\x00\x00"));
        assert!(ImageRenderer::is_gif(b"GIF87a\x00\x00"));
    }

    #[test]
    fn test_is_gif_with_non_gif_data() {
        assert!(!ImageRenderer::is_gif(b"\x89PNG\r\n\x1a\n"));
        assert!(!ImageRenderer::is_gif(b"\xff\xd8\xff\xe0"));
        assert!(!ImageRenderer::is_gif(b""));
    }

    #[test]
    fn test_is_gif_with_exactly_3_bytes() {
        assert!(!ImageRenderer::is_gif(b"GIF"));
        assert!(ImageRenderer::is_gif(b"GIF8"));
    }

    #[test]
    fn test_scaling_filter_to_filter_type_conversion() {
        assert!(matches!(
            FilterType::from(ScalingFilter::Nearest),
            FilterType::Nearest
        ));
        assert!(matches!(
            FilterType::from(ScalingFilter::Triangle),
            FilterType::Triangle
        ));
        assert!(matches!(
            FilterType::from(ScalingFilter::CatmullRom),
            FilterType::CatmullRom
        ));
        assert!(matches!(
            FilterType::from(ScalingFilter::Gaussian),
            FilterType::Gaussian
        ));
        assert!(matches!(
            FilterType::from(ScalingFilter::Lanczos3),
            FilterType::Lanczos3
        ));
    }
}
