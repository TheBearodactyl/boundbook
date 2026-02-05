use {
    arboard::{Clipboard, ImageData},
    boundbook::{BbfReader, Result, types::MediaType},
    clap::{Args, ValueEnum},
    color_eyre::eyre::Context,
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
    miette::miette,
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
    std::{
        io::{self, Cursor, Write},
        path::{Path, PathBuf},
        thread,
        time::Duration,
    },
};

#[derive(Args, Clone)]
#[command(disable_help_flag = true, author = "The Motherfucking Bearodactyl")]
pub struct ReadArgs {
    /// BBF file to read
    input: PathBuf,

    /// Pre-render all pages before reading (uses more memory but smoother navigation)
    #[arg(long)]
    prerender: bool,

    /// Maximum width in pixels (aspect ratio preserved)
    #[arg(long, value_name = "PIXELS")]
    max_width: Option<u32>,

    /// Maximum height in pixels (aspect ratio preserved)
    #[arg(long, value_name = "PIXELS")]
    max_height: Option<u32>,

    /// Maximum width in terminal columns (overrides max-width if set)
    #[arg(long, value_name = "COLS")]
    max_cols: Option<u16>,

    /// Maximum height in terminal rows (overrides max-height if set)
    #[arg(long, value_name = "ROWS")]
    max_rows: Option<u16>,

    /// Image scaling filter quality
    #[arg(long, value_enum, default_value = "lanczos3")]
    filter: ScalingFilter,

    /// Enable GIF animation playback
    #[arg(long)]
    enable_gif_animation: bool,

    /// GIF animation frame delay multiplier (1.0 = normal speed)
    #[arg(long, default_value = "1.0", value_name = "MULTIPLIER")]
    gif_speed: f32,

    /// Loop GIFs infinitely
    #[arg(long)]
    gif_loop: bool,

    /// Disable status bar
    #[arg(long)]
    no_status_bar: bool,
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
    )
    .with_context(|| format!("Failed to open BBF file: {}", args.input.display()))?;

    reader
        .run(args.prerender)
        .context("Error while running reader")?;

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
    gif_state: Option<GifAnimationState>,
}

#[derive(Clone)]
struct GifAnimationState {
    is_playing: bool,
    current_frame: usize,
    frame_count: usize,
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
    ) -> Result<Self> {
        let reader = BbfReader::open(path)?;
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
            gif_state: None,
        })
    }

    pub fn run(&mut self, prerender: bool) -> Result<()> {
        if prerender {
            self.prerender_all_pages()?;
        }

        terminal::enable_raw_mode()?;
        execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;

        let result = self.reader_loop(prerender);

        execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show)?;
        terminal::disable_raw_mode()?;

        result
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
        let (term_cols, term_rows) = terminal::size()?;
        let pb = ProgressBar::new(page_count as u64);

        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message("Pre-rendering pages...");

        let pages = self.reader.pages()?;
        let assets = self.reader.assets()?;
        let mut page_data: Vec<(usize, Vec<u8>, MediaType)> = Vec::with_capacity(page_count);

        for (i, page) in pages.iter().enumerate() {
            let asset = &assets[page.asset_index as usize];
            let data = self.reader.get_asset_data(asset)?.to_vec();
            let media_type = MediaType::from(asset.media_type);
            page_data.push((i, data, media_type));
        }

        let (max_width, max_height) = self.calculate_dimensions(term_cols, term_rows);
        let filter = self.filter;
        let enable_gif = self.enable_gif_animation;
        let pb_clone = pb.clone();
        let results: Vec<(usize, String)> = page_data
            .par_iter()
            .map(|(idx, data, media_type)| {
                let sixel_result = if enable_gif && Self::is_gif(data) {
                    Self::render_gif_first_frame_static(data, max_width, max_height, filter)
                } else {
                    Self::render_sixel_static(data, *media_type, max_width, max_height, filter)
                };

                let nidx = idx + 1;
                let sixel_data = match sixel_result {
                    Ok(s) => s,
                    Err(e) => format!("\r\nError rendering page {}: {}\r\n", nidx, e),
                };

                pb_clone.inc(1);
                (*idx, sixel_data)
            })
            .collect();

        pb.finish_with_message("Pre-rendering complete!");

        let mut sorted_results = results;
        sorted_results.sort_by_key(|(idx, _)| *idx);

        self.page_cache = sorted_results.into_iter().map(|(_, s)| s).collect();

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

        let cursor = Cursor::new(data.to_vec());
        let mut decoder = decode_options
            .read_info(cursor)
            .map_err(|e| format!("Failed to decode GIF: {}", e))?;

        let mut screen: GifScreen = GifScreen::new_decoder(&decoder);

        if let Some(frame) = decoder
            .read_next_frame()
            .map_err(|e| format!("Failed to read GIF frame: {}", e))?
        {
            screen
                .blit_frame(frame)
                .map_err(|e| format!("Failed to blit GIF frame: {}", e))?;

            let pixels = screen.pixels_rgba();
            let rgba_vec = pixels.to_contiguous_buf();
            let width = pixels.width() as u32;
            let height = pixels.height() as u32;
            let rgba_data: Vec<u8> = rgba_vec
                .0
                .iter()
                .flat_map(|rgba| [rgba.r, rgba.g, rgba.b, rgba.a])
                .collect();

            Self::render_rgba_to_sixel(
                &rgba_data,
                width,
                height,
                max_pixel_width,
                max_pixel_height,
                filter,
            )
        } else {
            Err("GIF has no frames".to_string().into())
        }
    }

    fn render_rgba_to_sixel(
        rgba_data: &[u8],
        width: u32,
        height: u32,
        max_pixel_width: u32,
        max_pixel_height: u32,
        filter: FilterType,
    ) -> Result<String> {
        let width_ratio = max_pixel_width as f32 / width as f32;
        let height_ratio = max_pixel_height as f32 / height as f32;
        let scale_ratio = width_ratio.min(height_ratio);
        let new_width = (width as f32 * scale_ratio) as u32;
        let new_height = (height as f32 * scale_ratio) as u32;
        let img_buffer = image::RgbaImage::from_raw(width, height, rgba_data.to_vec())
            .ok_or_else(|| "Failed to create RGBA image buffer".to_string())?;
        let scaled_img =
            image::imageops::resize(&img_buffer, new_width.max(1), new_height.max(1), filter);
        let (final_width, final_height) = scaled_img.dimensions();
        let sixel_img = SixelImage::from_rgba(
            scaled_img.into_raw(),
            final_width as usize,
            final_height as usize,
        );
        let sixel_data = sixel_img
            .encode()
            .map_err(|e| format!("Failed to encode sixel: {}", e))?;

        Ok(sixel_data)
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
            .map_err(|e| format!("Failed to guess image format: {}", e))?
            .decode()
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
    }

    #[allow(clippy::arithmetic_side_effects)]
    fn render_gif_animation(&mut self) -> Result<()> {
        let pages = self.reader.pages()?;
        if self.current_page >= pages.len() {
            return Ok(());
        }

        let page = &pages[self.current_page];
        let assets = self.reader.assets()?;
        let asset = &assets[page.asset_index as usize];
        let data = self.reader.get_asset_data(asset)?;

        if !Self::is_gif(data) {
            return Ok(());
        }

        let mut decode_options = DecodeOptions::new();
        decode_options.set_color_output(gif::ColorOutput::Indexed);

        let cursor = Cursor::new(data);
        let mut decoder = decode_options
            .read_info(cursor)
            .map_err(|e| format!("Failed to decode GIF: {}", e))?;

        let mut screen: GifScreen = GifScreen::new_decoder(&decoder);
        let (term_cols, term_rows) = terminal::size()?;
        let (max_width, max_height) = self.calculate_dimensions(term_cols, term_rows);
        let mut frame_count = 0usize;
        let mut frames_data = Vec::new();

        while let Some(frame) = decoder
            .read_next_frame()
            .map_err(|e| format!("Failed to read GIF frame: {}", e))?
        {
            screen
                .blit_frame(frame)
                .map_err(|e| format!("Failed to blit GIF frame: {}", e))?;

            let pixels = screen.pixels_rgba();
            let rgba_vec = pixels.to_contiguous_buf();
            let width = pixels.width() as u32;
            let height = pixels.height() as u32;
            let rgba_data: Vec<u8> = rgba_vec
                .0
                .iter()
                .flat_map(|rgba| [rgba.r, rgba.g, rgba.b, rgba.a])
                .collect();

            let delay = frame.delay as u32 * 10;
            let adjusted_delay = (delay as f32 * self.gif_speed) as u64;

            let sixel = Self::render_rgba_to_sixel(
                &rgba_data,
                width,
                height,
                max_width,
                max_height,
                self.filter,
            )?;

            frames_data.push((sixel, adjusted_delay));
            frame_count += 1;
        }

        self.gif_state = Some(GifAnimationState {
            is_playing: true,
            current_frame: 0,
            frame_count,
        });

        while let Some(ref mut state) = self.gif_state.clone() {
            if !state.is_playing {
                break;
            }

            let (sixel, delay) = &frames_data[state.current_frame];

            execute!(
                io::stdout(),
                terminal::Clear(ClearType::All),
                cursor::MoveTo(0, 0)
            )?;

            print!("{}", sixel);

            if !self.no_status_bar {
                self.render_gif_status_bar()?;
            }

            io::stdout().flush()?;

            if crossterm::event::poll(Duration::from_millis(*delay))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        self.gif_state = None;
                        return Ok(());
                    }
                    KeyCode::Char(' ') => {
                        state.is_playing = !state.is_playing;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.gif_state = None;
                        self.next_page();
                        return Ok(());
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        self.gif_state = None;
                        self.prev_page();
                        return Ok(());
                    }
                    _ => {}
                }
            }

            state.current_frame = (state.current_frame + 1) % frame_count;

            if !self.gif_loop && state.current_frame == 0 {
                state.is_playing = false;
            }
        }

        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn render_gif_status_bar(&self) -> Result<()> {
        let (_, height) = terminal::size()?;
        let rh = height - 1;
        execute!(io::stdout(), cursor::MoveTo(0, rh))?;

        if let Some(ref state) = self.gif_state {
            let status = if state.is_playing {
                "Playing"
            } else {
                "Paused"
            };
            let newframe = state.current_frame + 1;
            print!(
                "\rGIF: Frame {}/{} [{}] | [Space: pause/play] [h/l: page] [q: quit]",
                newframe, state.frame_count, status
            );
        }

        Ok(())
    }

    fn copy_image_to_clipboard(&self) -> Result<()> {
        let pages = self.reader.pages()?;
        if self.current_page >= pages.len() {
            return Err(miette!("Current page index out of bounds").into());
        }

        let page = &pages[self.current_page];
        let assets = self.reader.assets()?;
        let asset = &assets[page.asset_index as usize];
        let data = self.reader.get_asset_data(asset)?;
        let img = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .context("Failed to guess image format")?
            .decode()
            .context("Failed to decode image")?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let img_data = ImageData {
            width: width as usize,
            height: height as usize,
            bytes: rgba.into_raw().into(),
        };

        let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;

        clipboard
            .set_image(img_data)
            .context("Failed to copy image to clipboard")?;

        Ok(())
    }

    fn reader_loop(&mut self, prerender: bool) -> Result<()> {
        self.render_page(prerender)?;

        loop {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press && !self.handle_key(key, prerender)? {
                        break;
                    }
                }
                Event::Resize(_, _) => {
                    execute!(
                        io::stdout(),
                        terminal::Clear(ClearType::All),
                        cursor::MoveTo(0, 0)
                    )?;
                    println!(
                        "\r\nTerminal resized! Please restart the reader for proper scaling.\r\n"
                    );
                    println!("Press 'q' to quit...");
                    io::stdout().flush()?;
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
                        self.show_notification("✓ Page copied to clipboard")?;
                    }
                    Err(e) => {
                        self.show_notification(&format!("✗ Failed to copy: {}", e))?;
                    }
                }
                should_render = true;
            }

            KeyCode::Char('a') if self.enable_gif_animation => {
                let pages = self.reader.pages()?;
                if self.current_page < pages.len() {
                    let page = &pages[self.current_page];
                    let assets = self.reader.assets()?;
                    let asset = &assets[page.asset_index as usize];
                    let data = self.reader.get_asset_data(asset)?;

                    if Self::is_gif(data) {
                        self.render_gif_animation()?;
                        should_render = true;
                    } else {
                        self.show_notification("Current page is not a GIF")?;
                        should_render = true;
                    }
                }
            }

            _ => {}
        }

        if should_render {
            self.render_page(prerender)?;
        }

        Ok(true)
    }

    #[macroni_n_cheese::mathinator2000]
    fn show_notification(&self, message: &str) -> Result<()> {
        if self.no_status_bar {
            return Ok(());
        }

        let (_, height) = terminal::size()?;
        let rh = height - 2;

        execute!(
            io::stdout(),
            cursor::MoveTo(0, rh),
            terminal::Clear(ClearType::CurrentLine)
        )?;

        print!("\r{}", message);
        io::stdout().flush()?;

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
        )?;

        if prerender {
            if self.current_page >= self.page_cache.len() {
                return Ok(());
            }
            print!("{}", self.page_cache[self.current_page]);
        } else {
            let pages = self.reader.pages()?;
            if self.current_page >= pages.len() {
                return Ok(());
            }

            let page = &pages[self.current_page];
            let assets = self.reader.assets()?;
            let asset = &assets[page.asset_index as usize];

            let data = self.reader.get_asset_data(asset)?;
            let media_type = MediaType::from(asset.media_type);

            let (term_cols, term_rows) = terminal::size()?;
            let (max_width, max_height) = self.calculate_dimensions(term_cols, term_rows);

            let is_gif = Self::is_gif(data);

            match if is_gif && self.enable_gif_animation {
                Self::render_gif_first_frame_static(data, max_width, max_height, self.filter)
            } else {
                Self::render_sixel_static(data, media_type, max_width, max_height, self.filter)
            } {
                Ok(sixel_data) => {
                    print!("{}", sixel_data);
                }
                Err(e) => {
                    let npage = self.current_page + 1;
                    println!("\r\nError rendering page {}: {}\r\n", npage, e);
                }
            }
        }

        if !self.no_status_bar {
            self.render_status_bar()?;
        }

        io::stdout().flush()?;

        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn render_status_bar(&mut self) -> Result<()> {
        let (_, height) = terminal::size()?;
        let rh = height - 1;
        execute!(io::stdout(), cursor::MoveTo(0, rh))?;

        let nextpage = self.current_page + 1;
        let page_info = format!("Page {}/{}", nextpage, self.reader.page_count());

        let section_info = if let Some(idx) = self.current_section {
            let sections = self.reader.sections()?;
            let title = self.reader.get_string(sections[idx].section_title_offset)?;
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
        )?;

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

        io::stdout().flush()?;
        event::read()?;
        Ok(())
    }

    #[macroni_n_cheese::mathinator2000]
    fn show_info(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

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
                    let key = self.reader.get_string(meta.key_offset)?;
                    let val = self.reader.get_string(meta.value_offset)?;
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
                    let title = self.reader.get_string(section.section_title_offset)?;
                    let next_section = section.section_start_index + 1;
                    println!("  {} (Page {})", title, next_section);
                }
                println!();
            }
        }

        println!("Press any key to return...");
        io::stdout().flush()?;
        event::read()?;
        Ok(())
    }
}
