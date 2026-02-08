use {
    boundbook::{BbfReader, Result},
    clap::Args,
    interpolate::InterpolationMethod,
    miette::{IntoDiagnostic, miette},
    render::{RenderConfig, ScalingFilter},
    std::path::PathBuf,
    tui::TuiApp,
};

mod interpolate;
mod render;
mod state;
mod tui;

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
    #[arg(long, short = 'g', default_value = "true")]
    enable_gif_animation: bool,

    /// GIF animation frame delay multiplier (1.0 = normal speed)
    #[arg(long, default_value = "1.0", value_name = "MULTIPLIER")]
    gif_speed: f32,

    /// Loop GIFs infinitely
    #[arg(long, default_value = "true", short = 'l')]
    gif_loop: bool,

    /// Number of interpolated frames to generate between each GIF frame (0 = disabled)
    #[arg(long, default_value = "0", value_name = "COUNT", short = 'i')]
    gif_interpolate: usize,

    /// Frame interpolation algorithm
    #[arg(long, value_enum, default_value = "blend", short = 'm')]
    interpolation_method: InterpolationMethod,

    /// Sidebar width in columns
    #[arg(long, default_value = "30")]
    sidebar_width: u16,

    /// Slideshow auto-advance delay in seconds
    #[arg(long, default_value = "5.0", value_name = "SECONDS")]
    slideshow_delay: f32,
}

pub fn execute(args: ReadArgs) -> Result<()> {
    let dispinf = display_info::DisplayInfo::all().into_diagnostic()?;
    let first_disp = dispinf
        .first()
        .ok_or_else(|| miette!("Failed to get display info"))?;
    let height = args
        .max_height
        .unwrap_or((first_disp.height as f32 * 0.975).floor() as u32);
    let render_config = RenderConfig {
        max_width_pixels: args.max_width,
        max_height_pixels: Some(height),
        max_cols: args.max_cols,
        max_rows: args.max_rows,
        filter: args.filter.into(),
        enable_gif_animation: args.enable_gif_animation,
        gif_speed: args.gif_speed,
        gif_loop: args.gif_loop,
        gif_interpolate: args.gif_interpolate,
        interpolation_method: args.interpolation_method,
    };
    let reader = BbfReader::open(&args.input).into_diagnostic()?;
    let mut app = TuiApp::new(
        reader,
        render_config,
        args.sidebar_width,
        args.slideshow_delay,
        args.input.clone(),
    )?;

    app.run(args.prerender)?;

    Ok(())
}

pub struct BookReader {
    pub reader: BbfReader,
    pub current_page: usize,
    pub current_section: Option<usize>,
    pub page_cache: Vec<String>,
}

impl BookReader {
    #[macroni_n_cheese::mathinator2000]
    pub fn next_page(&mut self) {
        if self.current_page < self.reader.page_count().saturating_sub(1) as usize {
            self.current_page += 1;
            self.update_current_section();
        }
    }

    #[macroni_n_cheese::mathinator2000]
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.update_current_section();
        }
    }

    pub fn jump_to_page(&mut self, page: usize) {
        let max_page = (self.reader.page_count() as usize).saturating_sub(1);
        self.current_page = page.min(max_page);
        self.update_current_section();
    }

    #[macroni_n_cheese::mathinator2000]
    pub fn next_section(&mut self) {
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
    pub fn prev_section(&mut self) {
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

    pub const fn page_count(&self) -> usize {
        self.reader.page_count() as usize
    }

    pub fn get_section_info(&self) -> Option<String> {
        if let Some(idx) = self.current_section
            && let Ok(sections) = self.reader.sections()
            && let Ok(title) = self.reader.get_string(sections[idx].section_title_offset)
        {
            return Some(title.to_string());
        }
        None
    }
}
