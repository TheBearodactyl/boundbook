use rayon::iter::IntoParallelIterator;

use {
    arboard::{Clipboard, ImageData},
    boundbook::{BbfError, BbfReader, Result, types::MediaType},
    clap::ValueEnum,
    gif::DecodeOptions,
    gif_dispose::Screen as GifScreen,
    icy_sixel::SixelImage,
    image::{ImageReader, imageops::FilterType},
    indicatif::{ProgressBar, ProgressStyle},
    miette::{Context, IntoDiagnostic},
    rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
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
    pub max_width_pixels: Option<u32>,
    pub max_height_pixels: Option<u32>,
    pub max_cols: Option<u16>,
    pub max_rows: Option<u16>,
    pub filter: FilterType,
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

    #[macroni_n_cheese::mathinator2000]
    pub fn calculate_dimensions(
        &self,
        term_cols: u16,
        term_rows: u16,
        sidebar_width: u16,
    ) -> (u32, u32) {
        let effective_cols = self
            .config
            .max_cols
            .unwrap_or(term_cols)
            .saturating_sub(sidebar_width);
        let effective_rows = self.config.max_rows.unwrap_or(term_rows).saturating_sub(2);

        let term_max_width = effective_cols as u32 * 12;
        let term_max_height = effective_rows as u32 * 24;

        let max_width = self
            .config
            .max_width_pixels
            .unwrap_or(term_max_width)
            .min(term_max_width);
        let max_height = self
            .config
            .max_height_pixels
            .unwrap_or(term_max_height)
            .min(term_max_height);

        (max_width, max_height)
    }

    pub fn is_gif(data: &[u8]) -> bool {
        data.len() > 3 && &data[0..3] == b"GIF"
    }

    pub fn render_gif_first_frame_static(
        data: &[u8],
        max_pixel_width: u32,
        max_pixel_height: u32,
        filter: FilterType,
    ) -> Result<String> {
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

            Self::render_rgba_to_sixel(
                &rgba_data,
                width as u32,
                height as u32,
                max_pixel_width,
                max_pixel_height,
                filter,
            )
        } else {
            Err(BbfError::Other {
                message: "GIF contains no frames".to_string(),
            })
        }
    }

    pub fn render_rgba_to_sixel(
        rgba_data: &[u8],
        width: u32,
        height: u32,
        max_pixel_width: u32,
        max_pixel_height: u32,
        filter: FilterType,
    ) -> Result<String> {
        if width <= max_pixel_width && height <= max_pixel_height {
            let sixel_img =
                SixelImage::from_rgba(rgba_data.to_vec(), width as usize, height as usize);
            return sixel_img
                .encode()
                .into_diagnostic()
                .context("Failed to encode sixel")
                .map_err(|e| e.into());
        }

        let scale_ratio =
            (max_pixel_width as f32 / width as f32).min(max_pixel_height as f32 / height as f32);

        let new_width = ((width as f32 * scale_ratio) as u32).max(1);
        let new_height = ((height as f32 * scale_ratio) as u32).max(1);

        let img_buffer = image::RgbaImage::from_raw(width, height, rgba_data.to_vec())
            .ok_or_else(|| "Failed to create RGBA image buffer".to_string())?;

        let scaled_img = image::imageops::resize(&img_buffer, new_width, new_height, filter);
        let (final_width, final_height) = scaled_img.dimensions();

        let sixel_img = SixelImage::from_rgba(
            scaled_img.into_raw(),
            final_width as usize,
            final_height as usize,
        );

        sixel_img
            .encode()
            .into_diagnostic()
            .context("Failed to encode sixel")
            .map_err(|e| e.into())
    }

    pub fn render_sixel_static(
        data: &[u8],
        _media_type: MediaType,
        max_pixel_width: u32,
        max_pixel_height: u32,
        filter: FilterType,
    ) -> Result<String> {
        let img = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .into_diagnostic()
            .map_err(|e| format!("Failed to guess image format: {}", e))?
            .decode()
            .into_diagnostic()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        Self::render_rgba_to_sixel(
            &rgba.into_raw(),
            width,
            height,
            max_pixel_width,
            max_pixel_height,
            filter,
        )
        .into_diagnostic()
        .map_err(BbfError::from)
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

    /// Optimized GIF frame decoding with proper frame ordering
    ///
    /// # Panics
    ///
    /// Panics if GIF decoding fails
    #[allow(clippy::arithmetic_side_effects)]
    pub fn decode_gif_frames(
        &self,
        data: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Result<Vec<(String, u64)>> {
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
        let filter = self.config.filter;
        let interpolator = FrameInterpolator::new(self.config.interpolation_method);

        let mut frames_data: Vec<(String, u64)> =
            Vec::with_capacity(raw_frames.len() * (interpolate_count + 1));

        for i in 0..raw_frames.len() {
            let (rgba_data, width, height, delay) = &raw_frames[i];
            let adjusted_delay = (*delay as f32 * gif_speed) as u64;
            let frame_delay = if interpolate_count > 0 {
                adjusted_delay / (interpolate_count + 1) as u64
            } else {
                adjusted_delay
            };

            let sixel = Self::render_rgba_to_sixel(
                rgba_data, *width, *height, max_width, max_height, filter,
            )
            .into_diagnostic()
            .context(format!("Failed to render frame {} to sixel", i))?;

            frames_data.push((sixel, frame_delay));

            if interpolate_count > 0 {
                let next_idx = (i + 1) % raw_frames.len();
                let (next_rgba, next_width, next_height, _) = &raw_frames[next_idx];

                if *width == *next_width && *height == *next_height {
                    let interp_frames: std::result::Result<Vec<String>, miette::Report> =
                        (1..=interpolate_count)
                            .into_par_iter()
                            .map(|interp_i| {
                                let t = interp_i as f32 / (interpolate_count + 1) as f32;
                                let interpolated_rgba = interpolator.interpolate_frames(
                                    rgba_data, next_rgba, *width, *height, t,
                                );

                                Self::render_rgba_to_sixel(
                                    &interpolated_rgba,
                                    *width,
                                    *height,
                                    max_width,
                                    max_height,
                                    filter,
                                )
                                .into_diagnostic()
                                .context(format!(
                                    "Failed to render interpolated frame {}-{}",
                                    i, interp_i
                                ))
                            })
                            .collect();

                    for interp_sixel in interp_frames.map_err(BbfError::from)? {
                        frames_data.push((interp_sixel, frame_delay));
                    }
                }
            }
        }

        Ok(frames_data)
    }

    /// Pre-renders all pages in the book
    ///
    /// # Panics
    ///
    /// Panics if indicatif fails to parse the progress bar template or if page rendering fails
    #[macroni_n_cheese::mathinator2000]
    pub fn prerender_all_pages(
        &self,
        reader: &BbfReader,
        term_cols: u16,
        term_rows: u16,
        sidebar_width: u16,
    ) -> Result<Vec<String>> {
        let page_count = reader.page_count() as usize;
        let (max_width, max_height) =
            self.calculate_dimensions(term_cols, term_rows, sidebar_width);

        let pb = ProgressBar::new(page_count as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
                .into_diagnostic()?
                .progress_chars("#>-")
        );
        pb.set_message("Pre-rendering pages...");

        let pages = reader.pages().into_diagnostic()?;
        let assets = reader.assets().into_diagnostic()?;
        let filter = self.config.filter;
        let enable_gif = self.config.enable_gif_animation;
        let pb_clone = pb.clone();

        let results: Vec<String> = pages
            .par_iter()
            .enumerate()
            .map(|(idx, page)| {
                let asset = &assets[page.asset_index as usize];
                let data = reader
                    .get_asset_data(asset)
                    .into_diagnostic()
                    .unwrap_or_else(
                        #[allow(clippy::arithmetic_side_effects)]
                        |e| {
                            pb_clone.inc(1);
                            panic!("\r\nError loading page {}: {}\r\n", idx + 1, e);
                        },
                    );

                let sixel_result = if enable_gif && Self::is_gif(data) {
                    Self::render_gif_first_frame_static(data, max_width, max_height, filter)
                } else {
                    Self::render_sixel_static(
                        data,
                        MediaType::from(asset.media_type),
                        max_width,
                        max_height,
                        filter,
                    )
                };

                pb_clone.inc(1);

                sixel_result.unwrap_or_else(
                    #[allow(clippy::arithmetic_side_effects)]
                    |e| format!("\r\nError rendering page {}: {}\r\n", idx + 1, e),
                )
            })
            .collect();

        pb.finish_with_message("Pre-rendering complete!");
        Ok(results)
    }
}
