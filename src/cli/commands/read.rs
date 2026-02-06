use {
    arboard::{Clipboard, ImageData},
    boundbook::{BbfError, BbfReader, Result, types::MediaType},
    clap::{Args, ValueEnum},
    crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{self, ClearType},
    },
    gif::DecodeOptions,
    gif_dispose::Screen as GifScreen,
    icy_sixel::SixelImage,
    image::{ImageReader, imageops::FilterType},
    indicatif::{ProgressBar, ProgressStyle},
    lerp::Lerp,
    miette::{Context, IntoDiagnostic, miette},
    nalgebra::{Matrix2, Vector2},
    rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
    std::{
        fmt::Write as FmtWrite,
        io::{self, Cursor, Write},
        path::{Path, PathBuf},
        thread,
        time::Duration,
    },
};

#[derive(Args, Clone)]
#[command(author = "The Motherfucking Bearodactyl")]
pub struct ReadArgs {
    /// BBF file to read
    input: PathBuf,

    /// Pre-render all pages before reading (uses more memory but smoother navigation)
    #[arg(long, short = 'P')]
    prerender: bool,

    /// Maximum width in pixels (aspect ratio preserved)
    #[arg(long, value_name = "PIXELS", short = 'W')]
    max_width: Option<u32>,

    /// Maximum height in pixels (aspect ratio preserved)
    #[arg(long, value_name = "PIXELS", short = 'H')]
    max_height: Option<u32>,

    /// Maximum width in terminal columns (overrides max-width if set)
    #[arg(long, value_name = "COLS")]
    max_cols: Option<u16>,

    /// Maximum height in terminal rows (overrides max-height if set)
    #[arg(long, value_name = "ROWS")]
    max_rows: Option<u16>,

    /// Image scaling filter quality
    #[arg(long, value_enum, default_value = "lanczos3", short = 'f')]
    filter: ScalingFilter,

    /// Enable GIF animation playback
    #[arg(long, short = 'g')]
    enable_gif_animation: bool,

    /// GIF animation frame delay multiplier (1.0 = normal speed)
    #[arg(long, default_value = "1.0", value_name = "MULTIPLIER")]
    gif_speed: f32,

    /// Loop GIFs infinitely
    #[arg(long, default_value = "true", short = 'l')]
    gif_loop: bool,

    /// Disable status bar
    #[arg(long, short = 's')]
    no_status_bar: bool,

    /// Number of interpolated frames to generate between each GIF frame (0 = disabled)
    #[arg(long, default_value = "0", value_name = "COUNT", short = 'i')]
    gif_interpolate: usize,

    /// Frame interpolation algorithm
    #[arg(long, value_enum, default_value = "blend", short = 'm')]
    interpolation_method: InterpolationMethod,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InterpolationMethod {
    /// Simple linear blending (fastest)
    Blend,
    /// Weighted blending with ease-in/ease-out
    Smooth,
    /// Cosine interpolation for smoother transitions
    Cosine,
    /// Cubic hermite spline interpolation
    Cubic,
    /// Perlin smoothstep (quintic hermite)
    Perlin,
    /// Exponential ease-in-out
    Exponential,
    /// Optical flow based (Lucas-Kanade sparse)
    OpticalFlowSparse,
    /// Motion-compensated blending (simplified Horn-Schunck)
    MotionCompensated,
    /// Catmull-Rom spline (requires 4 frames, falls back to cubic)
    CatmullRom,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ScalingFilter {
    /// Nearest neighbor (fastest, lowest quality)
    Nearest,
    /// Linear/Triangle filter (fast)
    Triangle,
    /// Cubic/CatmullRom filter (balanced)
    CatmullRom,
    /// Gaussian filter (smooth)
    Gaussian,
    /// Lanczos3 filter (slowest, highest quality)
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

pub fn execute(args: ReadArgs) -> Result<()> {
    let mut reader = BookReader::new(
        &args.input,
        args.max_width,
        args.max_height,
        args.max_cols,
        args.max_rows,
        args.filter,
        args.enable_gif_animation,
        args.gif_speed,
        args.gif_loop,
        args.no_status_bar,
        args.gif_interpolate,
        args.interpolation_method,
    )
    .into_diagnostic()?;

    reader.run(args.prerender).into_diagnostic()?;

    Ok(())
}

pub struct BookReader {
    reader: BbfReader,
    current_page: usize,
    current_section: Option<usize>,
    page_cache: Vec<String>,
    max_width_pixels: Option<u32>,
    max_height_pixels: Option<u32>,
    max_cols: Option<u16>,
    max_rows: Option<u16>,
    filter: FilterType,
    enable_gif_animation: bool,
    gif_speed: f32,
    gif_loop: bool,
    no_status_bar: bool,
    gif_interpolate: usize,
    interpolation_method: InterpolationMethod,
}

impl BookReader {
    #[allow(clippy::too_many_arguments)]
    pub fn new<P: AsRef<Path>>(
        path: P,
        max_width: Option<u32>,
        max_height: Option<u32>,
        max_cols: Option<u16>,
        max_rows: Option<u16>,
        filter: ScalingFilter,
        enable_gif_animation: bool,
        gif_speed: f32,
        gif_loop: bool,
        no_status_bar: bool,
        gif_interpolate: usize,
        interpolation_method: InterpolationMethod,
    ) -> Result<Self> {
        let reader = BbfReader::open(path).into_diagnostic()?;
        Ok(Self {
            reader,
            current_page: 0,
            current_section: None,
            page_cache: Vec::new(),
            max_width_pixels: max_width,
            max_height_pixels: max_height,
            max_cols,
            max_rows,
            filter: filter.into(),
            enable_gif_animation,
            gif_speed,
            gif_loop,
            no_status_bar,
            gif_interpolate,
            interpolation_method,
        })
    }

    pub fn run(&mut self, prerender: bool) -> Result<()> {
        if prerender {
            self.prerender_all_pages().into_diagnostic()?;
        }

        terminal::enable_raw_mode().into_diagnostic()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide).into_diagnostic()?;

        let result = self.reader_loop(prerender);

        execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show).into_diagnostic()?;
        terminal::disable_raw_mode().into_diagnostic()?;

        result
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn ease_function(&self, t: f32, method: InterpolationMethod) -> f32 {
        match method {
            InterpolationMethod::Blend => t,
            InterpolationMethod::Smooth => t * t * (3.0 - 2.0 * t),
            InterpolationMethod::Cosine => (1.0 - f32::cos(t * std::f32::consts::PI)) / 2.0,
            InterpolationMethod::Cubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let f = 2.0 * t - 2.0;
                    1.0 + f * f * f / 2.0
                }
            }
            InterpolationMethod::Perlin => t * t * t * (t * (t * 6.0 - 15.0) + 10.0),
            InterpolationMethod::Exponential => {
                if t < 0.5 {
                    0.5 * f32::powf(2.0, 20.0 * t - 10.0)
                } else {
                    1.0 - 0.5 * f32::powf(2.0, -20.0 * t + 10.0)
                }
            }
            InterpolationMethod::OpticalFlowSparse
            | InterpolationMethod::MotionCompensated
            | InterpolationMethod::CatmullRom => t,
        }
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn compute_gradients(
        &self,
        data: &[u8],
        width: usize,
        height: usize,
        x: usize,
        y: usize,
    ) -> (f32, f32) {
        let idx = (y * width + x) * 4;

        if x == 0 || y == 0 || x >= width - 1 || y >= height - 1 {
            return (0.0, 0.0);
        }

        let to_gray = |idx: usize| -> f32 {
            let r = data[idx] as f32;
            let g = data[idx + 1] as f32;
            let b = data[idx + 2] as f32;
            0.299 * r + 0.587 * g + 0.114 * b
        };

        let left = to_gray(idx - 4);
        let right = to_gray(idx + 4);
        let top = to_gray(idx - width * 4);
        let bottom = to_gray(idx + width * 4);

        let gx = (right - left) / 2.0;
        let gy = (bottom - top) / 2.0;

        (gx, gy)
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn compute_optical_flow_sparse(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: usize,
        height: usize,
    ) -> Vec<(usize, usize, f32, f32)> {
        let mut flow_vectors = Vec::new();
        let window_size = 5;
        let stride = 16;

        for y in (window_size..height - window_size).step_by(stride) {
            for x in (window_size..width - window_size).step_by(stride) {
                let mut a_mat = Matrix2::zeros();
                let mut b_vec = Vector2::zeros();

                for dy in -(window_size as i32)..=(window_size as i32) {
                    for dx in -(window_size as i32)..=(window_size as i32) {
                        let px = (x as i32 + dx) as usize;
                        let py = (y as i32 + dy) as usize;

                        let (gx, gy) = self.compute_gradients(frame1, width, height, px, py);

                        let idx1 = (py * width + px) * 4;
                        let idx2 = (py * width + px) * 4;

                        let i1 = frame1[idx1] as f32;
                        let i2 = frame2[idx2] as f32;
                        let it = i2 - i1;

                        a_mat[(0, 0)] += gx * gx;
                        a_mat[(0, 1)] += gx * gy;
                        a_mat[(1, 0)] += gx * gy;
                        a_mat[(1, 1)] += gy * gy;

                        b_vec[0] -= gx * it;
                        b_vec[1] -= gy * it;
                    }
                }

                if let Some(inv) = a_mat.try_inverse() {
                    let flow = inv * b_vec;
                    let vx: f32 = flow[0];
                    let vy: f32 = flow[1];

                    if vx.abs() < 10.0 && vy.abs() < 10.0 {
                        flow_vectors.push((x, y, vx, vy));
                    }
                }
            }
        }

        flow_vectors
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn interpolate_with_optical_flow(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: usize,
        height: usize,
        t: f32,
    ) -> Vec<u8> {
        let flow = self.compute_optical_flow_sparse(frame1, frame2, width, height);
        let mut result = frame1.to_vec();

        for (fx, fy, vx, vy) in flow {
            let radius = 8;

            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let x = (fx as i32 + dx).clamp(0, width as i32 - 1) as usize;
                    let y = (fy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                    let src_x = (x as f32 - vx * t).clamp(0.0, width as f32 - 1.0);
                    let src_y = (y as f32 - vy * t).clamp(0.0, height as f32 - 1.0);
                    let x0 = src_x.floor() as usize;
                    let x1 = (x0 + 1).min(width - 1);
                    let y0 = src_y.floor() as usize;
                    let y1 = (y0 + 1).min(height - 1);
                    let wx = src_x - x0 as f32;
                    let wy = src_y - y0 as f32;
                    let idx = (y * width + x) * 4;

                    for c in 0..4 {
                        let v00 = frame2[(y0 * width + x0) * 4 + c] as f32;
                        let v01 = frame2[(y0 * width + x1) * 4 + c] as f32;
                        let v10 = frame2[(y1 * width + x0) * 4 + c] as f32;
                        let v11 = frame2[(y1 * width + x1) * 4 + c] as f32;

                        let v0 = v00.lerp(v01, wx);
                        let v1 = v10.lerp(v11, wx);
                        let v = v0.lerp(v1, wy);
                        let orig = frame1[idx + c] as f32;
                        result[idx + c] = orig.lerp(v, t) as u8;
                    }
                }
            }
        }

        result
    }

    /// Motion-compensated interpolation (simplified Horn-Schunck)
    #[allow(clippy::arithmetic_side_effects)]
    fn interpolate_motion_compensated(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: usize,
        height: usize,
        t: f32,
    ) -> Vec<u8> {
        let mut result = vec![0u8; frame1.len()];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = (y * width + x) * 4;

                for c in 0..4 {
                    let (gx, _) = self.compute_gradients(frame1, width, height, x, y);
                    let i1 = frame1[idx + c] as f32;
                    let i2 = frame2[idx + c] as f32;
                    let it = i2 - i1;
                    let motion = if gx.abs() > 0.1 { -it / gx * t } else { 0.0 };
                    let src_x = (x as f32 + motion).clamp(0.0, width as f32 - 1.0);
                    let v1 = i1;
                    let v2 = self.bilinear_sample(frame2, width, height, src_x, y as f32, c);

                    result[idx + c] = v1.lerp(v2, t) as u8;
                }
            }
        }

        result
    }

    /// Bilinear sampling helper
    #[allow(clippy::arithmetic_side_effects)]
    fn bilinear_sample(
        &self,
        data: &[u8],
        width: usize,
        height: usize,
        x: f32,
        y: f32,
        channel: usize,
    ) -> f32 {
        let x0 = x.floor() as usize;
        let x1 = (x0 + 1).min(width - 1);
        let y0 = y.floor() as usize;
        let y1 = (y0 + 1).min(height - 1);

        let wx = x - x0 as f32;
        let wy = y - y0 as f32;

        let v00 = data[(y0 * width + x0) * 4 + channel] as f32;
        let v01 = data[(y0 * width + x1) * 4 + channel] as f32;
        let v10 = data[(y1 * width + x0) * 4 + channel] as f32;
        let v11 = data[(y1 * width + x1) * 4 + channel] as f32;

        let v0 = v00.lerp(v01, wx);
        let v1 = v10.lerp(v11, wx);
        v0.lerp(v1, wy)
    }

    /// # Panics
    ///
    /// panics if the 2 frames aren't the same size
    fn interpolate_frames(
        &self,
        frame1: &[u8],
        frame2: &[u8],
        width: u32,
        height: u32,
        t: f32,
        method: InterpolationMethod,
    ) -> Vec<u8> {
        assert_eq!(frame1.len(), frame2.len());

        match method {
            InterpolationMethod::OpticalFlowSparse => self.interpolate_with_optical_flow(
                frame1,
                frame2,
                width as usize,
                height as usize,
                t,
            ),

            InterpolationMethod::MotionCompensated => self.interpolate_motion_compensated(
                frame1,
                frame2,
                width as usize,
                height as usize,
                t,
            ),

            _ => {
                let adjusted_t = self.ease_function(t, method);

                frame1
                    .iter()
                    .zip(frame2.iter())
                    .map(|(a, b)| {
                        let a_f = *a as f32;
                        let b_f = *b as f32;
                        a_f.lerp(b_f, adjusted_t) as u8
                    })
                    .collect()
            }
        }
    }

    /// # Panics
    ///
    /// panics if it fails to add 1 to the interpolation count (should never happen)
    #[allow(clippy::too_many_arguments, clippy::arithmetic_side_effects)]
    fn generate_interpolated_frames(
        &self,
        frame1_rgba: &[u8],
        frame2_rgba: &[u8],
        width: u32,
        height: u32,
        max_width: u32,
        max_height: u32,
        interpolate_count: usize,
    ) -> Result<Vec<String>> {
        let mut interpolated_sixels = Vec::with_capacity(interpolate_count);

        for i in 1..=interpolate_count {
            let t = i as f32 / (interpolate_count + 1) as f32;

            let interpolated_rgba = self.interpolate_frames(
                frame1_rgba,
                frame2_rgba,
                width,
                height,
                t,
                self.interpolation_method,
            );

            let sixel = Self::render_rgba_to_sixel(
                &interpolated_rgba,
                width,
                height,
                max_width,
                max_height,
                self.filter,
            )
            .into_diagnostic()
            .context(format!("Failed to render interpolated frame {}", i))?;

            interpolated_sixels.push(sixel);
        }

        Ok(interpolated_sixels)
    }

    #[macroni_n_cheese::mathinator2000]
    fn calculate_dimensions(&self, term_cols: u16, term_rows: u16) -> (u32, u32) {
        let effective_cols = self.max_cols.unwrap_or(term_cols);
        let effective_rows = if self.no_status_bar {
            self.max_rows.unwrap_or(term_rows)
        } else {
            self.max_rows.unwrap_or(term_rows.saturating_sub(2))
        };

        let term_max_width = effective_cols as u32 * 12;
        let term_max_height = effective_rows as u32 * 24;

        let max_width = self
            .max_width_pixels
            .unwrap_or(term_max_width)
            .min(term_max_width);
        let max_height = self
            .max_height_pixels
            .unwrap_or(term_max_height)
            .min(term_max_height);

        (max_width, max_height)
    }

    #[macroni_n_cheese::mathinator2000]
    /// # Panics
    ///
    /// panics if indicatif fails to parse the progress bar template
    fn prerender_all_pages(&mut self) -> Result<()> {
        let page_count = self.reader.page_count() as usize;
        let (term_cols, term_rows) = terminal::size().into_diagnostic()?;

        self.page_cache = Vec::with_capacity(page_count);

        let pb = ProgressBar::new(page_count as u64);
        pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
            .into_diagnostic()?
            .progress_chars("#>-")
    );
        pb.set_message("Pre-rendering pages...");

        let pages = self.reader.pages().into_diagnostic()?;
        let assets = self.reader.assets().into_diagnostic()?;

        let (max_width, max_height) = self.calculate_dimensions(term_cols, term_rows);
        let filter = self.filter;
        let enable_gif = self.enable_gif_animation;
        let pb_clone = pb.clone();

        let results: Vec<String> = pages
            .par_iter()
            .enumerate()
            .map(|(idx, page)| {
                let asset = &assets[page.asset_index as usize];
                let data = self
                    .reader
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
        self.page_cache = results;

        println!("Press any key to start reading...");
        thread::sleep(Duration::from_millis(500));

        Ok(())
    }

    fn is_gif(data: &[u8]) -> bool {
        data.len() > 3 && &data[0..3] == b"GIF"
    }

    fn render_gif_first_frame_static(
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

    #[allow(clippy::arithmetic_side_effects)]
    fn render_gif_animation(&mut self) -> Result<()> {
        let pages = self.reader.pages().into_diagnostic()?;
        if self.current_page >= pages.len() {
            return Ok(());
        }

        let page = &pages[self.current_page];
        let assets = self.reader.assets().into_diagnostic()?;
        let asset = &assets[page.asset_index as usize];
        let data = self.reader.get_asset_data(asset).into_diagnostic()?;

        if !Self::is_gif(data) {
            return Ok(());
        }

        let mut decode_options = DecodeOptions::new();
        decode_options.set_color_output(gif::ColorOutput::Indexed);

        let cursor = Cursor::new(data);
        let mut decoder = decode_options
            .read_info(cursor)
            .into_diagnostic()
            .context("Failed to decode GIF for animation")?;
        let mut screen = GifScreen::new_decoder(&decoder);
        let (term_cols, term_rows) = terminal::size().into_diagnostic()?;
        let (max_width, max_height) = self.calculate_dimensions(term_cols, term_rows);
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

            let rgba_data: Vec<u8> = rgba_vec
                .iter()
                .flat_map(|rgba| [rgba.r, rgba.g, rgba.b, rgba.a])
                .collect();

            raw_frames.push((rgba_data, width as u32, height as u32, delay));
        }

        if raw_frames.is_empty() {
            return Err(BbfError::Other {
                message: "GIF animation contains no frames".to_string(),
            });
        }

        let mut frames_data: Vec<(String, u64)> = Vec::new();
        let interpolate_count = self.gif_interpolate;

        for i in 0..raw_frames.len() {
            let (rgba_data, width, height, delay) = &raw_frames[i];
            let adjusted_delay = (*delay as f32 * self.gif_speed) as u64;
            let sixel = Self::render_rgba_to_sixel(
                rgba_data,
                *width,
                *height,
                max_width,
                max_height,
                self.filter,
            )
            .into_diagnostic()
            .context(format!("Failed to render frame {} to sixel", i))?;

            let frame_delay = if interpolate_count > 0 {
                adjusted_delay / (interpolate_count + 1) as u64
            } else {
                adjusted_delay
            };

            frames_data.push((sixel, frame_delay));

            if interpolate_count > 0 {
                let next_idx = (i + 1) % raw_frames.len();
                let (next_rgba, next_width, next_height, _) = &raw_frames[next_idx];

                if *width == *next_width && *height == *next_height {
                    let interpolated = self
                        .generate_interpolated_frames(
                            rgba_data,
                            next_rgba,
                            *width,
                            *height,
                            max_width,
                            max_height,
                            interpolate_count,
                        )
                        .into_diagnostic()?;

                    for interp_sixel in interpolated {
                        frames_data.push((interp_sixel, frame_delay));
                    }
                }
            }
        }

        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .into_diagnostic()?;

        let frame_count = frames_data.len();
        let original_frame_count = raw_frames.len();
        let mut current_frame = 0;
        let mut is_playing = true;
        let mut last_frame_time = std::time::Instant::now();

        loop {
            if !is_playing && !self.gif_loop && current_frame == 0 {
                break;
            }

            let (sixel, target_delay) = &frames_data[current_frame];

            let elapsed = last_frame_time.elapsed().as_millis() as u64;
            let actual_delay = if elapsed < *target_delay {
                target_delay - elapsed
            } else {
                0
            };

            let mut output = String::with_capacity(sixel.len() + 300);

            write!(output, "\x1b[H").into_diagnostic()?;
            output.push_str(sixel);

            if !self.no_status_bar {
                Self::render_status_with_progress(
                    &mut output,
                    current_frame,
                    frame_count,
                    original_frame_count,
                    is_playing,
                    interpolate_count > 0,
                )?;
            }

            print!("{}", output);
            io::stdout().flush().into_diagnostic()?;

            last_frame_time = std::time::Instant::now();

            if crossterm::event::poll(Duration::from_millis(actual_delay)).into_diagnostic()?
                && let Event::Key(key) = event::read().into_diagnostic()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        execute!(
                            io::stdout(),
                            terminal::Clear(ClearType::All),
                            cursor::MoveTo(0, 0)
                        )
                        .into_diagnostic()?;
                        return Ok(());
                    }
                    KeyCode::Char(' ') => is_playing = !is_playing,
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.next_page();
                        return Ok(());
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.prev_page();
                        return Ok(());
                    }
                    _ => {}
                }
            }

            if is_playing {
                current_frame = (current_frame + 1) % frame_count;

                if !self.gif_loop && current_frame == 0 {
                    is_playing = false;
                }
            }
        }

        Ok(())
    }

    fn render_status_with_progress(
        output: &mut String,
        current_frame: usize,
        total_frames: usize,
        original_frames: usize,
        is_playing: bool,
        interpolated: bool,
    ) -> Result<()> {
        let (term_width, height) = terminal::size().into_diagnostic()?;

        let progress = (current_frame as f64 / total_frames as f64 * 100.0) as usize;

        let bar_width = (term_width as usize).saturating_sub(50).max(20);
        let filled = (bar_width as f64 * (current_frame as f64 / total_frames as f64)) as usize;
        let empty = bar_width.saturating_sub(filled);

        let status_icon = if is_playing { "▶" } else { "⏸" };
        let status_text = if is_playing { "Playing" } else { "Paused" };

        let bar_visual = format!(
            "\x1b[36m█\x1b[0m{}\x1b[90m{}\x1b[36m█\x1b[0m",
            "▓".repeat(filled.saturating_sub(1)),
            "░".repeat(empty.saturating_sub(1))
        );

        write!(output, "\x1b[{};1H\x1b[K", height.saturating_sub(1)).into_diagnostic()?;

        let interp_info = if interpolated {
            format!(
                " \x1b[32m[Interpolated: {}→{}]\x1b[0m",
                original_frames, total_frames
            )
        } else {
            String::new()
        };

        write!(
            output,
            "{} Frame \x1b[1m{}/{}\x1b[0m {} {}% {}{}",
            status_icon,
            current_frame.saturating_add(1),
            total_frames,
            bar_visual,
            progress,
            status_text,
            interp_info
        )
        .into_diagnostic()?;

        write!(output, "\x1b[{};1H\x1b[K", height).into_diagnostic()?;
        write!(
            output,
            "\x1b[2m[Space: pause/play] [h/l: page] [q: quit]\x1b[0m"
        )
        .into_diagnostic()?;

        Ok(())
    }

    fn render_rgba_to_sixel(
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

    fn render_sixel_static(
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

    fn copy_image_to_clipboard(&self) -> Result<()> {
        let pages = self.reader.pages().into_diagnostic()?;
        if self.current_page >= pages.len() {
            return Err(miette!("Current page index out of bounds").into());
        }

        let page = &pages[self.current_page];
        let assets = self.reader.assets().into_diagnostic()?;
        let asset = &assets[page.asset_index as usize];
        let data = self.reader.get_asset_data(asset).into_diagnostic()?;
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

    fn reader_loop(&mut self, prerender: bool) -> Result<()> {
        self.render_page(prerender)?;

        loop {
            match event::read().into_diagnostic()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press
                        && !self.handle_key(key, prerender).into_diagnostic()?
                    {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    execute!(
                        io::stdout(),
                        terminal::Clear(ClearType::All),
                        cursor::MoveTo(0, 0)
                    )
                    .into_diagnostic()?;
                    println!(
                        "\r\nTerminal resized! Please restart the reader for proper scaling.\r\n"
                    );
                    println!("Press 'q' to quit...");
                    io::stdout().flush().into_diagnostic()?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn handle_key(&mut self, key: KeyEvent, prerender: bool) -> Result<bool> {
        let mut should_render = false;

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(false);
            }

            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') | KeyCode::PageDown => {
                self.next_page();
                should_render = true;
            }

            KeyCode::Left | KeyCode::Char('h') | KeyCode::PageUp => {
                self.prev_page();
                should_render = true;
            }

            KeyCode::Char('n') | KeyCode::Char(']') => {
                self.next_section();
                should_render = true;
            }

            KeyCode::Char('p') | KeyCode::Char('[') => {
                self.prev_section();
                should_render = true;
            }

            KeyCode::Home | KeyCode::Char('g') => {
                self.current_page = 0;
                should_render = true;
            }

            KeyCode::End | KeyCode::Char('G') => {
                self.current_page = self.reader.page_count().saturating_sub(1) as usize;
                should_render = true;
            }

            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.current_page = (self.current_page + 10)
                    .min(self.reader.page_count().saturating_sub(1) as usize);
                should_render = true;
            }

            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.current_page = self.current_page.saturating_sub(10);
                should_render = true;
            }

            KeyCode::Char('?') => {
                self.show_help()?;
                should_render = true;
            }

            KeyCode::Char('i') => {
                self.show_info()?;
                should_render = true;
            }

            KeyCode::Char('y') => {
                match self.copy_image_to_clipboard() {
                    Ok(_) => {
                        self.show_notification("✓ Page copied to clipboard")
                            .into_diagnostic()?;
                    }
                    Err(e) => {
                        self.show_notification(&format!("✗ Failed to copy: {}", e))
                            .into_diagnostic()?;
                    }
                }
                should_render = true;
            }

            KeyCode::Char('a') if self.enable_gif_animation => {
                let pages = self.reader.pages().into_diagnostic()?;
                if self.current_page < pages.len() {
                    let page = &pages[self.current_page];
                    let assets = self.reader.assets().into_diagnostic()?;
                    let asset = &assets[page.asset_index as usize];
                    let data = self.reader.get_asset_data(asset).into_diagnostic()?;

                    if Self::is_gif(data) {
                        self.render_gif_animation().into_diagnostic()?;
                        should_render = true;
                    } else {
                        self.show_notification("Current page is not a GIF")
                            .into_diagnostic()?;
                        should_render = true;
                    }
                }
            }

            _ => {}
        }

        if should_render {
            self.render_page(prerender).into_diagnostic()?;
        }

        Ok(true)
    }

    #[macroni_n_cheese::mathinator2000]
    fn show_notification(&self, message: &str) -> Result<()> {
        if self.no_status_bar {
            return Ok(());
        }

        let (_, height) = terminal::size().into_diagnostic()?;
        let rh = height - 2;

        execute!(
            io::stdout(),
            cursor::MoveTo(0, rh),
            terminal::Clear(ClearType::CurrentLine)
        )
        .into_diagnostic()?;

        print!("\r{}", message);
        io::stdout().flush().into_diagnostic()?;

        thread::sleep(Duration::from_millis(1500));

        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn next_page(&mut self) {
        if self.current_page < self.reader.page_count().saturating_sub(1) as usize {
            self.current_page += 1;
            self.update_current_section();
        }
    }

    #[macroni_n_cheese::mathinator2000]
    fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.update_current_section();
        }
    }

    #[macroni_n_cheese::mathinator2000]
    fn next_section(&mut self) {
        let sections_res = self.reader.sections();

        if let Ok(sections) = sections_res {
            if sections.is_empty() {
                return;
            }

            let current_idx = self.current_section.unwrap_or(0);
            if current_idx + 1 < sections.len() {
                self.current_page = sections[current_idx + 1].section_start_index as usize;
                self.current_section = Some(current_idx + 1);
            }
        }
    }

    #[macroni_n_cheese::mathinator2000]
    fn prev_section(&mut self) {
        let sections_res = self.reader.sections();

        if let Ok(sections) = sections_res {
            if sections.is_empty() {
                return;
            }

            let current_idx = self.current_section.unwrap_or(0);
            if current_idx > 0 {
                self.current_page = sections[current_idx - 1].section_start_index as usize;
                self.current_section = Some(current_idx - 1);
            }
        }
    }

    fn update_current_section(&mut self) {
        let sections_res = self.reader.sections();

        if let Ok(sections) = sections_res {
            if sections.is_empty() {
                self.current_section = None;
                return;
            }

            for (i, section) in sections.iter().enumerate().rev() {
                if section.section_start_index as usize <= self.current_page {
                    self.current_section = Some(i);
                    return;
                }
            }
            self.current_section = None;
        }
    }

    #[macroni_n_cheese::mathinator2000]
    fn render_page(&mut self, prerender: bool) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .into_diagnostic()?;

        if prerender {
            if let Some(cached_page) = self.page_cache.get(self.current_page) {
                print!("{}", cached_page);
            } else {
                return Ok(());
            }
        } else {
            let pages = self.reader.pages().into_diagnostic()?;
            if self.current_page >= pages.len() {
                return Ok(());
            }

            let page = &pages[self.current_page];
            let assets = self.reader.assets().into_diagnostic()?;
            let asset = &assets[page.asset_index as usize];

            let data = self.reader.get_asset_data(asset).into_diagnostic()?;
            let media_type = MediaType::from(asset.media_type);

            let (term_cols, term_rows) = terminal::size().into_diagnostic()?;
            let (max_width, max_height) = self.calculate_dimensions(term_cols, term_rows);

            let is_gif = Self::is_gif(data);

            let render_result = if is_gif && self.enable_gif_animation {
                Self::render_gif_first_frame_static(data, max_width, max_height, self.filter)
            } else {
                Self::render_sixel_static(data, media_type, max_width, max_height, self.filter)
            };

            let new_page = self.current_page + 1;
            match render_result.into_diagnostic() {
                Ok(sixel_data) => print!("{}", sixel_data),
                Err(e) => println!("\r\nError rendering page {}: {}\r\n", new_page, e),
            }
        }

        if !self.no_status_bar {
            self.render_status_bar().into_diagnostic()?;
        }

        io::stdout().flush().into_diagnostic()?;

        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn render_status_bar(&mut self) -> Result<()> {
        let (_, height) = terminal::size().into_diagnostic()?;
        let rh = height - 1;
        execute!(io::stdout(), cursor::MoveTo(0, rh)).into_diagnostic()?;

        let nextpage = self.current_page + 1;
        let page_info = format!("Page {}/{}", nextpage, self.reader.page_count());

        let section_info = if let Some(idx) = self.current_section {
            let sections = self.reader.sections()?;
            let title = self
                .reader
                .get_string(sections[idx].section_title_offset)
                .into_diagnostic()?;
            format!(" | Section: {}", title)
        } else {
            String::new()
        };

        let gif_info = if self.enable_gif_animation {
            " | [a: play GIF]"
        } else {
            ""
        };

        print!(
            "\r{}{} | [h/l: page] [p/n: section] [q: quit] [?: help]{}",
            page_info, section_info, gif_info
        );

        Ok(())
    }

    fn show_help(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .into_diagnostic()?;

        println!("\r\n=== BBF Reader - Keyboard Controls ===\r\n");
        println!("Navigation:");
        println!("  h, ←, PgUp      - Previous page");
        println!("  l, →, Space, PgDn - Next page");
        println!("  p, [            - Previous section/chapter");
        println!("  n, ]            - Next section/chapter");
        println!("  g, Home         - First page");
        println!("  G, End          - Last page");
        println!("  Ctrl-j          - Jump forward 10 pages");
        println!("  Ctrl-k          - Jump backward 10 pages\r\n");
        println!("Other:");
        println!("  i               - Show book info");
        println!("  y               - Copy current page to clipboard");
        if self.enable_gif_animation {
            println!("  a               - Play GIF animation (if current page is GIF)");
        }
        println!("  ?               - Show this help");
        println!("  q, Esc, Ctrl-c  - Quit\r\n");
        println!("Press any key to return...");

        io::stdout().flush().into_diagnostic()?;
        event::read().into_diagnostic()?;
        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn show_info(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .into_diagnostic()?;

        println!("\r\n=== Book Information ===\r\n");
        println!("Pages:       {}", self.reader.page_count());
        println!("Assets:      {}", self.reader.asset_count());
        println!("BBF Version: {}\r\n", self.reader.version());

        println!("Metadata:");
        let metadata_res = self.reader.metadata();
        if let Ok(metadata) = metadata_res {
            if metadata.is_empty() {
                println!("  None\r\n");
            } else {
                for meta in metadata {
                    let key = self.reader.get_string(meta.key_offset).into_diagnostic()?;
                    let val = self
                        .reader
                        .get_string(meta.value_offset)
                        .into_diagnostic()?;
                    println!("  {}: {}", key, val);
                }
                println!();
            }
        }

        println!("Sections:");
        let sections_res = self.reader.sections();
        if let Ok(sections) = sections_res {
            if sections.is_empty() {
                println!("  None\r\n");
            } else {
                for section in sections {
                    let title = self
                        .reader
                        .get_string(section.section_title_offset)
                        .into_diagnostic()?;
                    let next_section = section.section_start_index + 1;
                    println!("  {} (Page {})", title, next_section);
                }
                println!();
            }
        }

        println!("Press any key to return...");
        io::stdout().flush().into_diagnostic()?;
        event::read().into_diagnostic()?;
        Ok(())
    }
}
