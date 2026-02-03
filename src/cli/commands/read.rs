use {
    arboard::Clipboard,
    boundbook::{BbfReader, MediaType, Result},
    clap::Args,
    color_eyre::eyre::Context,
    crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{self, ClearType},
    },
    icy_sixel::SixelImage,
    image::ImageReader,
    indicatif::{ProgressBar, ProgressStyle},
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
    std::{
        io::{self, Write},
        path::{Path, PathBuf},
    },
};

#[derive(Args)]
pub struct ReadArgs {
    /// BBF file to read
    input: PathBuf,

    /// Pre-render all pages before reading (uses more memory but smoother navigation)
    #[arg(long)]
    prerender: bool,
}

pub fn execute(args: ReadArgs) -> color_eyre::Result<()> {
    let mut reader = BookReader::new(&args.input)
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
}

impl BookReader {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = BbfReader::open(path)?;
        Ok(Self {
            reader,
            current_page: 0,
            current_section: None,
            page_cache: Vec::new(),
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

        let pages = self.reader.pages();
        let assets = self.reader.assets();

        let mut page_data: Vec<(usize, Vec<u8>, MediaType)> = Vec::with_capacity(page_count);
        for (i, page) in pages.iter().enumerate() {
            let asset = &assets[page.asset_index as usize];
            let data = self.reader.get_asset_data(asset).to_vec();
            let media_type = MediaType::from(asset.media_type);
            page_data.push((i, data, media_type));
        }

        let pb_clone = pb.clone();
        let results: Vec<(usize, String)> = page_data
            .par_iter()
            .map(|(idx, data, media_type)| {
                let sixel_result =
                    Self::render_sixel_static(data, *media_type, term_cols, term_rows);

                let sixel_data = match sixel_result {
                    Ok(s) => s,
                    Err(e) => format!("\r\nError rendering page {}: {}\r\n", idx + 1, e),
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
        std::thread::sleep(std::time::Duration::from_millis(500));

        Ok(())
    }

    fn render_sixel_static(
        data: &[u8],
        _media_type: MediaType,
        term_cols: u16,
        term_rows: u16,
    ) -> Result<String> {
        let img = ImageReader::new(io::Cursor::new(data))
            .with_guessed_format()
            .map_err(|e| format!("Failed to guess image format: {}", e))?
            .decode()
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let available_rows = term_rows.saturating_sub(2);

        let max_pixel_width = term_cols as u32 * 12;
        let max_pixel_height = available_rows as u32 * 24;

        let width_ratio = max_pixel_width as f32 / width as f32;
        let height_ratio = max_pixel_height as f32 / height as f32;

        let scale_ratio = width_ratio.min(height_ratio);

        let new_width = (width as f32 * scale_ratio) as u32;
        let new_height = (height as f32 * scale_ratio) as u32;

        let scaled_img = image::imageops::resize(
            &rgba,
            new_width.max(1),
            new_height.max(1),
            image::imageops::FilterType::Lanczos3,
        );

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

    #[allow(unused)]
    fn render_sixel_with_size(
        &self,
        data: &[u8],
        media_type: MediaType,
        term_cols: u16,
        term_rows: u16,
    ) -> Result<String> {
        Self::render_sixel_static(data, media_type, term_cols, term_rows)
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

            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let mut clipboard = Clipboard::new().expect("Failed to get kb");
                todo!()
            }

            _ => {}
        }

        if should_render {
            self.render_page(prerender)?;
        }

        Ok(true)
    }

    fn next_page(&mut self) {
        if self.current_page < self.reader.page_count().saturating_sub(1) as usize {
            self.current_page += 1;
            self.update_current_section();
        }
    }

    fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.update_current_section();
        }
    }

    fn next_section(&mut self) {
        let sections = self.reader.sections();
        if sections.is_empty() {
            return;
        }

        let current_idx = self.current_section.unwrap_or(0);
        if current_idx + 1 < sections.len() {
            self.current_page = sections[current_idx + 1].start_index as usize;
            self.current_section = Some(current_idx + 1);
        }
    }

    fn prev_section(&mut self) {
        let sections = self.reader.sections();
        if sections.is_empty() {
            return;
        }

        let current_idx = self.current_section.unwrap_or(0);
        if current_idx > 0 {
            self.current_page = sections[current_idx - 1].start_index as usize;
            self.current_section = Some(current_idx - 1);
        }
    }

    fn update_current_section(&mut self) {
        let sections = self.reader.sections();
        if sections.is_empty() {
            self.current_section = None;
            return;
        }

        for (i, section) in sections.iter().enumerate().rev() {
            if section.start_index as usize <= self.current_page {
                self.current_section = Some(i);
                return;
            }
        }
        self.current_section = None;
    }

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
            let pages = self.reader.pages();
            if self.current_page >= pages.len() {
                return Ok(());
            }

            let page = &pages[self.current_page];
            let assets = self.reader.assets();
            let asset = &assets[page.asset_index as usize];

            let data = self.reader.get_asset_data(asset);
            let media_type = MediaType::from(asset.media_type);

            let (term_cols, term_rows) = terminal::size()?;

            match Self::render_sixel_static(data, media_type, term_cols, term_rows) {
                Ok(sixel_data) => {
                    print!("{}", sixel_data);
                }
                Err(e) => {
                    println!(
                        "\r\nError rendering page {}: {}\r\n",
                        self.current_page + 1,
                        e
                    );
                }
            }
        }

        self.render_status_bar()?;

        io::stdout().flush()?;

        Ok(())
    }

    fn render_status_bar(&mut self) -> Result<()> {
        let (_, height) = terminal::size()?;

        execute!(io::stdout(), cursor::MoveTo(0, height - 1))?;

        let page_info = format!(
            "Page {}/{}",
            self.current_page + 1,
            self.reader.page_count()
        );

        let section_info = if let Some(idx) = self.current_section {
            let sections = self.reader.sections();
            let title = self.reader.get_string(sections[idx].title_offset)?;
            format!(" | Section: {}", title)
        } else {
            String::new()
        };

        print!(
            "\r{}{} | [h/l: page] [p/n: section] [q: quit] [?: help]",
            page_info, section_info
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
        println!("  h, â†, PgUp      - Previous page");
        println!("  l, â†’, Space, PgDn - Next page");
        println!("  p, [            - Previous section/chapter");
        println!("  n, ]            - Next section/chapter");
        println!("  g, Home         - First page");
        println!("  G, End          - Last page");
        println!("  Ctrl-j          - Jump forward 10 pages");
        println!("  Ctrl-k          - Jump backward 10 pages\r\n");
        println!("Other:");
        println!("  i               - Show book info");
        println!("  ?               - Show this help");
        println!("  q, Esc, Ctrl-c  - Quit\r\n");
        println!("Press any key to return...");

        io::stdout().flush()?;
        event::read()?;
        Ok(())
    }

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
        let metadata = self.reader.metadata();
        if metadata.is_empty() {
            println!("  None\r\n");
        } else {
            for meta in metadata {
                let key = self.reader.get_string(meta.key_offset)?;
                let val = self.reader.get_string(meta.val_offset)?;
                println!("  {}: {}", key, val);
            }
            println!();
        }

        println!("Sections:");
        let sections = self.reader.sections();
        if sections.is_empty() {
            println!("  None\r\n");
        } else {
            for section in sections {
                let title = self.reader.get_string(section.title_offset)?;
                println!("  {} (Page {})", title, section.start_index + 1);
            }
            println!();
        }

        println!("Press any key to return...");
        io::stdout().flush()?;
        event::read()?;
        Ok(())
    }
}
