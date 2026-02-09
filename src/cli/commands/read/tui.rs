#![allow(clippy::arithmetic_side_effects)]
use {
    super::{
        BookReader,
        render::{ImageRenderer, RenderConfig},
        state::{self, BookState},
    },
    boundbook::{BbfReader, Result, types::MediaType},
    crossterm::{
        cursor,
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
            KeyModifiers, MouseButton, MouseEventKind,
        },
        execute,
        terminal::{self},
    },
    image::DynamicImage,
    miette::IntoDiagnostic,
    ratatui::{
        Frame, Terminal,
        backend::CrosstermBackend,
        layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph, Wrap},
    },
    ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol},
    std::{
        collections::BTreeSet,
        io, panic,
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    },
    tui_tree_widget::{Tree, TreeItem, TreeState},
};

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            terminal::LeaveAlternateScreen,
            cursor::Show
        );
        let _ = terminal::disable_raw_mode();
    }
}

enum AppMode {
    Normal,
    GifAnimation {
        frames: Arc<Vec<(DynamicImage, u64)>>,
        current_frame: usize,
        is_playing: bool,
        last_frame_time: Instant,
        loop_count: usize,
    },
    GoToPage {
        input: String,
    },
    Slideshow {
        last_advance: Instant,
    },
}

pub struct TuiApp {
    book_reader: BookReader,
    renderer: ImageRenderer,
    picker: Picker,
    /// The current page's image protocol state for ratatui-image.
    current_image: Option<StatefulProtocol>,
    tree_state: TreeState<usize>,
    sidebar_width: u16,
    show_sidebar: bool,
    notification: Option<String>,
    notification_time: Option<Instant>,
    show_help: bool,
    mode: AppMode,
    bookmarks: BTreeSet<usize>,
    show_metadata: bool,
    show_bookmarks: bool,
    slideshow_delay_secs: f32,
    book_path: PathBuf,
}

impl TuiApp {
    pub fn new(
        reader: BbfReader,
        config: RenderConfig,
        picker: Picker,
        sidebar_width: u16,
        slideshow_delay_secs: f32,
        book_path: PathBuf,
    ) -> Result<Self> {
        let persisted = state::load_state(&book_path);

        let max_page = (reader.page_count() as usize).saturating_sub(1);
        let restored_page = persisted.current_page.min(max_page);

        let book_reader = BookReader {
            reader,
            current_page: restored_page,
            current_section: None,
        };

        let renderer = ImageRenderer::new(config);

        let mut tree_state = TreeState::default();
        tree_state.select_first();

        let mut app = Self {
            book_reader,
            renderer,
            picker,
            current_image: None,
            tree_state,
            sidebar_width,
            show_sidebar: true,
            notification: None,
            notification_time: None,
            show_help: false,
            mode: AppMode::Normal,
            bookmarks: persisted.bookmarks,
            show_metadata: false,
            show_bookmarks: false,
            slideshow_delay_secs,
            book_path,
        };

        app.load_current_page_image();

        Ok(app)
    }

    fn load_current_page_image(&mut self) {
        if let Some(dyn_img) = self.decode_current_page() {
            self.current_image = Some(self.picker.new_resize_protocol(dyn_img));
        } else {
            self.current_image = None;
        }
    }

    fn decode_current_page(&self) -> Option<DynamicImage> {
        let pages = self.book_reader.reader.pages().ok()?;
        let page = pages.get(self.book_reader.current_page)?;
        let assets = self.book_reader.reader.assets().ok()?;
        let asset = assets.get(page.asset_index as usize)?;
        let data = self.book_reader.reader.get_asset_data(asset).ok()?;

        let is_gif = ImageRenderer::is_gif(data);
        if is_gif && self.renderer.config.enable_gif_animation {
            ImageRenderer::decode_gif_first_frame(data).ok()
        } else {
            ImageRenderer::decode_image(data).ok()
        }
    }

    fn current_book_state(&self) -> BookState {
        BookState {
            current_page: self.book_reader.current_page,
            bookmarks: self.bookmarks.clone(),
            source_path: self.book_path.to_string_lossy().to_string(),
        }
    }

    fn save(&self) {
        let _ = state::save_state(&self.book_path, &self.current_book_state());
    }

    pub fn run(&mut self, _prerender: bool) -> Result<()> {
        if self.book_reader.current_page > 0 {
            self.notification = Some(format!(
                "Resumed at page {}",
                self.book_reader.current_page + 1
            ));
            self.notification_time = Some(Instant::now());
        }

        let original_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let _ = execute!(
                io::stdout(),
                DisableMouseCapture,
                terminal::LeaveAlternateScreen,
                cursor::Show
            );
            let _ = terminal::disable_raw_mode();
            original_hook(info);
        }));

        terminal::enable_raw_mode().into_diagnostic()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            terminal::EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )
        .into_diagnostic()?;

        let _guard = TerminalGuard;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).into_diagnostic()?;

        let result = self.main_loop(&mut terminal);

        self.save();

        drop(_guard);
        let _ = panic::take_hook();

        result
    }

    fn main_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            if let Some(t) = self.notification_time
                && t.elapsed() >= Duration::from_secs(3)
            {
                self.notification = None;
                self.notification_time = None;
            }

            terminal.draw(|f| self.render_ui(f)).into_diagnostic()?;

            if let AppMode::GifAnimation { .. } = &self.mode {
                if !self.handle_gif_animation(terminal)? {
                    break;
                }
                continue;
            }

            if let AppMode::Slideshow {
                ref mut last_advance,
            } = self.mode
            {
                let delay = Duration::from_secs_f32(self.slideshow_delay_secs);
                if last_advance.elapsed() >= delay {
                    *last_advance = Instant::now();
                    let at_end = self.book_reader.current_page
                        >= self.book_reader.page_count().saturating_sub(1);
                    if at_end {
                        self.mode = AppMode::Normal;
                        self.notification = Some("Slideshow finished".to_string());
                        self.notification_time = Some(Instant::now());
                    } else {
                        self.book_reader.next_page();
                        self.load_current_page_image();
                    }
                    continue;
                }
            }

            let poll_timeout = match &self.mode {
                AppMode::Slideshow { last_advance } => {
                    let delay = Duration::from_secs_f32(self.slideshow_delay_secs);
                    let elapsed = last_advance.elapsed();
                    if elapsed >= delay {
                        Duration::from_millis(1)
                    } else {
                        delay - elapsed
                    }
                }
                _ => Duration::from_millis(16),
            };

            if event::poll(poll_timeout).into_diagnostic()? {
                match event::read().into_diagnostic()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if !self.handle_key(key)? {
                            break;
                        }
                    }
                    Event::Mouse(mouse_event) => {
                        self.handle_mouse(mouse_event)?;
                    }
                    Event::Resize(_, _) => {
                        // StatefulImage handles re-encoding on area change automatically.
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.book_reader.next_page();
                self.load_current_page_image();
            }
            MouseEventKind::ScrollUp => {
                self.book_reader.prev_page();
                self.load_current_page_image();
            }
            MouseEventKind::Down(MouseButton::Left) if self.show_sidebar => {
                if mouse.column < self.sidebar_width {
                    self.tree_state
                        .click_at(Position::new(mouse.row, mouse.column));
                    let selected = self.tree_state.selected();
                    if let Some(&page) = selected.last() {
                        self.book_reader.jump_to_page(page);
                        self.load_current_page_image();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn bookmark_at_slot(&self, slot: usize) -> Option<usize> {
        self.bookmarks.iter().nth(slot).copied()
    }

    fn start_gif_animation(&mut self) -> Result<()> {
        let pages = self.book_reader.reader.pages().into_diagnostic()?;
        if self.book_reader.current_page >= pages.len() {
            return Ok(());
        }

        let page = &pages[self.book_reader.current_page];
        let assets = self.book_reader.reader.assets().into_diagnostic()?;
        let asset = &assets[page.asset_index as usize];
        let data = self
            .book_reader
            .reader
            .get_asset_data(asset)
            .into_diagnostic()?;

        if !ImageRenderer::is_gif(data) {
            self.notification = Some("Current page is not a GIF".to_string());
            self.notification_time = Some(Instant::now());
            return Ok(());
        }

        let frames = self.renderer.decode_gif_frames(data)?;
        if let Some((first_img, _)) = frames.first() {
            self.current_image = Some(self.picker.new_resize_protocol(first_img.clone()));
        }

        self.mode = AppMode::GifAnimation {
            frames: Arc::new(frames),
            current_frame: 0,
            is_playing: true,
            last_frame_time: Instant::now(),
            loop_count: 0,
        };

        Ok(())
    }

    fn handle_gif_animation(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<bool> {
        let poll_timeout = if let AppMode::GifAnimation {
            ref frames,
            current_frame,
            is_playing,
            ref last_frame_time,
            ..
        } = self.mode
        {
            if is_playing {
                let (_, target_delay) = &frames[current_frame];
                let elapsed_ms = last_frame_time.elapsed().as_millis() as u64;
                let remaining = target_delay.saturating_sub(elapsed_ms);
                Duration::from_millis(remaining.max(1))
            } else {
                Duration::from_millis(50)
            }
        } else {
            return Ok(true);
        };

        if event::poll(poll_timeout).into_diagnostic()? {
            match event::read().into_diagnostic()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let AppMode::GifAnimation {
                        ref mut is_playing, ..
                    } = self.mode
                    {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.mode = AppMode::Normal;
                                self.load_current_page_image();
                                return Ok(true);
                            }
                            KeyCode::Char(' ') => {
                                *is_playing = !*is_playing;
                            }
                            KeyCode::Right | KeyCode::Char('l') => {
                                self.mode = AppMode::Normal;
                                self.book_reader.next_page();
                                self.load_current_page_image();
                                return Ok(true);
                            }
                            KeyCode::Left | KeyCode::Char('h') => {
                                self.mode = AppMode::Normal;
                                self.book_reader.prev_page();
                                self.load_current_page_image();
                                return Ok(true);
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(_, _) => {
                    // ratatui-image re-encodes on area change; just redraw.
                }
                _ => {}
            }
        }

        // Advance frame if enough time has elapsed.
        let should_swap = if let AppMode::GifAnimation {
            ref frames,
            ref mut current_frame,
            ref mut is_playing,
            ref mut last_frame_time,
            ref mut loop_count,
            ..
        } = self.mode
        {
            let (_, target_delay) = &frames[*current_frame];
            let elapsed_ms = last_frame_time.elapsed().as_millis() as u64;

            if *is_playing && elapsed_ms >= *target_delay {
                *last_frame_time = Instant::now();
                let old_frame = *current_frame;
                *current_frame = (*current_frame + 1) % frames.len();

                if old_frame > *current_frame {
                    *loop_count += 1;
                }

                if !self.renderer.config.gif_loop && *current_frame == 0 && *loop_count > 0 {
                    *is_playing = false;
                }

                Some(*current_frame)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(frame_idx) = should_swap
            && let AppMode::GifAnimation { ref frames, .. } = self.mode
        {
            let (ref img, _) = frames[frame_idx];
            self.current_image = Some(self.picker.new_resize_protocol(img.clone()));
        }

        // Re-draw every tick so the UI stays responsive.
        terminal.draw(|f| self.render_ui(f)).into_diagnostic()?;

        Ok(true)
    }

    fn render_ui(&mut self, frame: &mut Frame) {
        let chunks = if self.show_sidebar {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(self.sidebar_width), Constraint::Min(0)])
                .split(frame.area())
                .to_vec()
        } else {
            vec![Rect::default(), frame.area()]
        };

        self.render_sidebar(frame, chunks[0]);
        self.render_content(frame, chunks[1]);
        self.render_status_bar(frame, chunks[1]);

        if let Some(ref msg) = self.notification {
            self.render_notification(frame, chunks[1], msg);
        }

        if self.show_help {
            self.render_help_overlay(frame);
        }

        if self.show_metadata {
            self.render_metadata_overlay(frame);
        }

        if self.show_bookmarks {
            self.render_bookmarks_overlay(frame);
        }

        if let AppMode::GoToPage { ref input } = self.mode {
            self.render_goto_page_dialog(frame, input);
        }

        if let AppMode::Slideshow { .. } = self.mode {
            self.render_slideshow_indicator(frame);
        }
    }

    /// # Panics
    ///
    /// panics if the tree widget fails to initialize
    fn render_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let items = self.build_tree_items();

        let tree_widget = Tree::new(&items)
            .expect("Failed to create tree widget")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("\u{f0669} Navigation")
                    .style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("\u{f0400} ");

        if self.show_sidebar {
            frame.render_stateful_widget(tree_widget, area, &mut self.tree_state);
        }
    }

    /// # Panics
    ///
    /// panics if it fails to initialize the tree widget
    fn build_tree_items(&self) -> Vec<TreeItem<'static, usize>> {
        // (unchanged from original -- omitted for brevity, keep as-is)
        if let Ok(sections) = self.book_reader.reader.sections()
            && !sections.is_empty()
        {
            let mut section_items = Vec::new();

            for (idx, section) in sections.iter().enumerate() {
                if let Ok(title) = self
                    .book_reader
                    .reader
                    .get_string(section.section_title_offset)
                {
                    let start_page = section.section_start_index as usize;
                    let end_page = sections
                        .get(idx + 1)
                        .map(|s| s.section_start_index as usize)
                        .unwrap_or(self.book_reader.page_count());

                    let mut page_items = Vec::new();
                    for page in start_page..end_page {
                        let bookmark_marker = if self.bookmarks.contains(&page) {
                            " *"
                        } else {
                            ""
                        };
                        let current_marker = if page == self.book_reader.current_page {
                            " <"
                        } else {
                            ""
                        };
                        page_items.push(TreeItem::new_leaf(
                            page,
                            format!("  Page {}{}{}", page + 1, bookmark_marker, current_marker),
                        ));
                    }

                    if let Ok(tree_item) =
                        TreeItem::new(start_page, format!("\u{f024b} {}", title), page_items)
                    {
                        section_items.push(tree_item);
                    }
                }
            }

            if !section_items.is_empty() {
                return section_items;
            }
        }

        (0..self.book_reader.page_count())
            .map(|page| {
                let bookmark_marker = if self.bookmarks.contains(&page) {
                    " *"
                } else {
                    ""
                };
                let current_marker = if page == self.book_reader.current_page {
                    " <"
                } else {
                    ""
                };
                TreeItem::new_leaf(
                    page,
                    format!(
                        "\u{f0309} Page {}{}{}",
                        page + 1,
                        bookmark_marker,
                        current_marker
                    ),
                )
            })
            .collect()
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        // Reserve the last row for the status bar.
        let image_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        // Render the image through ratatui-image's StatefulImage widget.
        if let Some(ref mut protocol) = self.current_image {
            let image_widget = StatefulImage::default();
            frame.render_stateful_widget(image_widget, image_area, protocol);
        }
    }

    // render_status_bar, render_notification, render_goto_page_dialog,
    // render_metadata_overlay, render_bookmarks_overlay, find_section_for_page,
    // render_slideshow_indicator, render_help_overlay -- all UNCHANGED from original.
    // (They only draw ratatui Paragraph / Block widgets and don't touch images.)

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let status_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1),
            width: area.width,
            height: 1,
        };

        let bookmark_indicator = if self.bookmarks.contains(&self.book_reader.current_page) {
            " [*] "
        } else {
            " "
        };

        let page_info = format!(
            " Page {}/{}{}",
            self.book_reader.current_page + 1,
            self.book_reader.page_count(),
            bookmark_indicator,
        );

        let section_info = self
            .book_reader
            .get_section_info()
            .map(|s| format!("| {} ", s))
            .unwrap_or_default();

        let gif_hint = if self.renderer.config.enable_gif_animation {
            self.book_reader
                .reader
                .pages()
                .ok()
                .and_then(|pages| pages.get(self.book_reader.current_page).copied())
                .and_then(|page| {
                    self.book_reader
                        .reader
                        .assets()
                        .ok()
                        .and_then(|assets| assets.get(page.asset_index as usize).copied())
                })
                .and_then(|asset| {
                    if asset.media_type == MediaType::Gif as u8 {
                        Some("| [a] GIF ")
                    } else {
                        None
                    }
                })
                .unwrap_or("")
        } else {
            ""
        };

        let slideshow_hint = match &self.mode {
            AppMode::Slideshow { .. } => "| SLIDESHOW ",
            _ => "",
        };

        let gif_status = match &self.mode {
            AppMode::GifAnimation {
                frames,
                current_frame,
                is_playing,
                loop_count,
                ..
            } => {
                let icon = if *is_playing { "\u{25b6}" } else { "\u{23f8}" };
                let loop_info = if *loop_count > 0 {
                    format!(" L{}", loop_count + 1)
                } else {
                    String::new()
                };
                format!(
                    "| {} {}/{}{} ",
                    icon,
                    current_frame + 1,
                    frames.len(),
                    loop_info,
                )
            }
            _ => String::new(),
        };

        let help = "| [:] GoTo | [?] Help | [q] Quit";
        let status_text = format!(
            "{}{}{}{}{}{}",
            page_info, section_info, gif_hint, gif_status, slideshow_hint, help
        );
        let status_bar = Paragraph::new(Line::from(vec![Span::styled(
            status_text,
            Style::default().fg(Color::Gray),
        )]));

        frame.render_widget(status_bar, status_area);
    }

    fn render_notification(&self, frame: &mut Frame, area: Rect, message: &str) {
        let notification_area = Rect {
            x: area.x + area.width / 4,
            y: area.y + area.height / 2,
            width: area.width / 2,
            height: 3,
        };

        let notification = Paragraph::new(message)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Notification")
                    .style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(notification, notification_area);
    }

    fn render_goto_page_dialog(&self, frame: &mut Frame, input: &str) {
        let area = frame.area();
        let popup_width = 40.min(area.width.saturating_sub(4));
        let popup_height = 5;
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let display_text = format!("Page (1-{}): {}_", self.book_reader.page_count(), input);

        let dialog = Paragraph::new(display_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Go to Page ")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });

        frame.render_widget(dialog, popup_area);
    }

    fn render_metadata_overlay(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_width = 60.min(area.width.saturating_sub(4));
        let popup_height = 20.min(area.height.saturating_sub(4));
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let mut lines: Vec<Line<'static>> = Vec::new();

        if let Ok(metadata) = self.book_reader.reader.metadata() {
            if metadata.is_empty() {
                lines.push(Line::from("  (no metadata in this file)"));
            } else {
                for entry in metadata {
                    let key = self
                        .book_reader
                        .reader
                        .get_string(entry.key_offset)
                        .unwrap_or("???");
                    let val = self
                        .book_reader
                        .reader
                        .get_string(entry.value_offset)
                        .unwrap_or("???");
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {}: ", key),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(val.to_string()),
                    ]));
                }
            }
        } else {
            lines.push(Line::from("  (failed to read metadata)"));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "  Pages: {}",
            self.book_reader.page_count()
        )));
        lines.push(Line::from(format!(
            "  Assets: {}",
            self.book_reader.reader.asset_count()
        )));
        lines.push(Line::from(format!(
            "  Format version: {}",
            self.book_reader.reader.version()
        )));

        if let Ok(sections) = self.book_reader.reader.sections() {
            lines.push(Line::from(format!("  Sections: {}", sections.len())));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("  Press [i] or [Esc] to close"));

        let panel = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Book Info ")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Magenta)),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });

        frame.render_widget(panel, popup_area);
    }

    fn render_bookmarks_overlay(&self, frame: &mut Frame) {
        let area = frame.area();
        let popup_width = 52.min(area.width.saturating_sub(4));
        let max_lines = self.bookmarks.len() + 8;
        let popup_height = (max_lines as u16).clamp(7, area.height.saturating_sub(4));
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let mut lines: Vec<Line<'static>> = Vec::new();

        if self.bookmarks.is_empty() {
            lines.push(Line::from("  (no bookmarks yet)"));
            lines.push(Line::from(""));
            lines.push(Line::from("  Press [b] on any page to bookmark it"));
        } else {
            for (slot, &page) in self.bookmarks.iter().enumerate() {
                let section_label = self.find_section_for_page(page).unwrap_or_default();
                let suffix = if !section_label.is_empty() {
                    format!("  ({})", section_label)
                } else {
                    String::new()
                };
                let current_marker = if page == self.book_reader.current_page {
                    " <-- here"
                } else {
                    ""
                };
                let keybind = if slot < 9 {
                    format!("[{}] ", slot + 1)
                } else {
                    "    ".to_string()
                };
                lines.push(Line::from(format!(
                    "  {}Page {}{}{}",
                    keybind,
                    page + 1,
                    suffix,
                    current_marker
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from("  Press [1]-[9] to jump, [B] or [Esc] to close"));

        let panel = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Bookmarks ")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow)),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });

        frame.render_widget(panel, popup_area);
    }

    fn find_section_for_page(&self, page: usize) -> Option<String> {
        if let Ok(sections) = self.book_reader.reader.sections() {
            for section in sections.iter().rev() {
                if section.section_start_index as usize <= page
                    && let Ok(title) = self
                        .book_reader
                        .reader
                        .get_string(section.section_title_offset)
                {
                    return Some(title.to_string());
                }
            }
        }
        None
    }

    fn render_slideshow_indicator(&self, frame: &mut Frame) {
        let area = frame.area();
        let indicator_width = 24.min(area.width);
        let indicator_area = Rect {
            x: area.width.saturating_sub(indicator_width).saturating_sub(1),
            y: 0,
            width: indicator_width,
            height: 3,
        };

        frame.render_widget(Clear, indicator_area);

        let text = format!("Slideshow ({:.1}s)", self.slideshow_delay_secs);
        let widget = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Green)),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(widget, indicator_area);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        self.notification = None;
        self.notification_time = None;

        if let AppMode::GoToPage { ref mut input } = self.mode {
            match key.code {
                KeyCode::Esc => {
                    self.mode = AppMode::Normal;
                }
                KeyCode::Enter => {
                    if let Ok(page_num) = input.parse::<usize>() {
                        if page_num >= 1 && page_num <= self.book_reader.page_count() {
                            self.book_reader.jump_to_page(page_num - 1);
                            self.mode = AppMode::Normal;
                            self.load_current_page_image();
                        } else {
                            self.notification = Some(format!(
                                "Page must be between 1 and {}",
                                self.book_reader.page_count()
                            ));
                            self.notification_time = Some(Instant::now());
                            self.mode = AppMode::Normal;
                        }
                    } else {
                        self.mode = AppMode::Normal;
                    }
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    input.push(c);
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                _ => {}
            }
            return Ok(true);
        }

        if let AppMode::Slideshow { .. } = self.mode {
            self.mode = AppMode::Normal;
            self.notification = Some("Slideshow stopped".to_string());
            self.notification_time = Some(Instant::now());
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s')
            ) {
                return Ok(true);
            }
        }

        if self.show_help {
            self.show_help = false;
            return Ok(true);
        }

        if self.show_metadata {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('i')) {
                self.show_metadata = false;
            }
            return Ok(true);
        }

        if self.show_bookmarks {
            match key.code {
                KeyCode::Esc | KeyCode::Char('B') => {
                    self.show_bookmarks = false;
                }
                KeyCode::Char(c @ '1'..='9') => {
                    let slot = (c as u8 - b'1') as usize;
                    if let Some(page) = self.bookmark_at_slot(slot) {
                        self.book_reader.jump_to_page(page);
                        self.show_bookmarks = false;
                        self.load_current_page_image();
                    }
                }
                _ => {}
            }
            return Ok(true);
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(false),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(false);
            }

            KeyCode::Tab => {
                self.show_sidebar = !self.show_sidebar;
            }

            KeyCode::Right if self.show_sidebar => {
                self.tree_state.key_right();
            }

            KeyCode::Left if self.show_sidebar => {
                self.tree_state.key_left();
            }

            KeyCode::Right | KeyCode::Char('l') => {
                self.book_reader.next_page();
                self.load_current_page_image();
            }

            KeyCode::Left | KeyCode::Char('h') => {
                self.book_reader.prev_page();
                self.load_current_page_image();
            }

            KeyCode::Char('n') | KeyCode::Char(']') => {
                self.book_reader.next_section();
                self.load_current_page_image();
            }

            KeyCode::Char('p') | KeyCode::Char('[') => {
                self.book_reader.prev_section();
                self.load_current_page_image();
            }

            KeyCode::Home | KeyCode::Char('g') => {
                self.book_reader.current_page = 0;
                self.load_current_page_image();
            }

            KeyCode::End | KeyCode::Char('G') => {
                self.book_reader.current_page = self.book_reader.page_count().saturating_sub(1);
                self.load_current_page_image();
            }

            KeyCode::Char('y') => {
                match self.renderer.copy_image_to_clipboard(
                    &self.book_reader.reader,
                    self.book_reader.current_page,
                ) {
                    Ok(()) => {
                        self.notification = Some("Page copied to clipboard".to_string());
                        self.notification_time = Some(Instant::now());
                    }
                    Err(e) => {
                        self.notification = Some(format!("Failed to copy: {}", e));
                        self.notification_time = Some(Instant::now());
                    }
                }
            }

            KeyCode::Char('a') if self.renderer.config.enable_gif_animation => {
                self.start_gif_animation()?;
            }

            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }

            KeyCode::Char(':') => {
                self.mode = AppMode::GoToPage {
                    input: String::new(),
                };
            }

            KeyCode::Char('i') => {
                self.show_metadata = !self.show_metadata;
            }

            KeyCode::Char('b') => {
                let page = self.book_reader.current_page;
                if self.bookmarks.contains(&page) {
                    self.bookmarks.remove(&page);
                    self.notification = Some(format!("Bookmark removed (page {})", page + 1));
                } else {
                    self.bookmarks.insert(page);
                    self.notification = Some(format!("Bookmarked page {}", page + 1));
                }
                self.notification_time = Some(Instant::now());
                self.save();
            }

            KeyCode::Char('B') => {
                self.show_bookmarks = !self.show_bookmarks;
            }

            KeyCode::Char(c @ '1'..='9') if !self.show_sidebar => {
                let slot = (c as u8 - b'1') as usize;
                if let Some(page) = self.bookmark_at_slot(slot) {
                    self.book_reader.jump_to_page(page);
                    self.notification = Some(format!(
                        "Jumped to bookmark {} (page {})",
                        slot + 1,
                        page + 1
                    ));
                    self.notification_time = Some(Instant::now());
                    self.load_current_page_image();
                }
            }

            KeyCode::Char('s') => {
                self.mode = AppMode::Slideshow {
                    last_advance: Instant::now(),
                };
                self.notification = Some(format!(
                    "Slideshow started ({:.1}s) -- any key stops",
                    self.slideshow_delay_secs
                ));
                self.notification_time = Some(Instant::now());
            }

            KeyCode::Enter if self.show_sidebar => {
                let selected = self.tree_state.selected();
                if let Some(&page) = selected.last() {
                    self.book_reader.jump_to_page(page);
                    self.load_current_page_image();
                }
            }

            KeyCode::Up if self.show_sidebar => {
                self.tree_state.key_up();
            }

            KeyCode::Down if self.show_sidebar => {
                self.tree_state.key_down();
            }

            KeyCode::Char(' ') if self.show_sidebar => {
                self.tree_state.toggle_selected();
            }

            _ => {}
        }

        Ok(true)
    }

    fn render_help_overlay(&self, frame: &mut Frame) {
        // (unchanged from original -- keep as-is)
        let area = frame.area();
        let popup_width = 64.min(area.width.saturating_sub(4));
        let popup_height = 42.min(area.height.saturating_sub(2));
        let popup_area = Rect {
            x: area.width.saturating_sub(popup_width) / 2,
            y: area.height.saturating_sub(popup_height) / 2,
            width: popup_width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let mut lines = vec![
            Line::from(Span::styled(
                "Navigation",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )),
            Line::from("  Tab               Toggle sidebar"),
            Line::from("  Up/Down           Navigate sidebar tree"),
            Line::from("  Left/Right        Expand/collapse tree sections"),
            Line::from("  Space             Toggle section expand/collapse"),
            Line::from("  Enter             Jump to selected page/section"),
            Line::from("  h                 Previous page"),
            Line::from("  l                 Next page"),
            Line::from("  p, [              Previous section"),
            Line::from("  n, ]              Next section"),
            Line::from("  g, Home           First page"),
            Line::from("  G, End            Last page"),
            Line::from("  :                 Go to page (type number)"),
            Line::from("  Scroll wheel      Previous/next page"),
            Line::from(""),
            Line::from(Span::styled(
                "Bookmarks",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from("  b                 Toggle bookmark on current page"),
            Line::from("  B                 Show bookmark list"),
            Line::from("  1-9               Jump to bookmark by slot number"),
            Line::from("                    (ordered by page number)"),
            Line::from(""),
            Line::from(Span::styled(
                "Info & Slideshow",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Magenta),
            )),
            Line::from("  i                 Show book metadata / info"),
            Line::from("  s                 Start slideshow (any key stops)"),
            Line::from(""),
        ];

        if self.renderer.config.enable_gif_animation {
            lines.push(Line::from(Span::styled(
                "Animation",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Green),
            )));
            lines.push(Line::from("  a                 Play GIF animation"));
            lines.push(Line::from("  Space (in GIF)    Pause/play"));
            lines.push(Line::from("  q/Esc (in GIF)    Exit animation"));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            "Other",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Gray),
        )));
        lines.push(Line::from("  y                 Copy page to clipboard"));
        lines.push(Line::from("  ?                 Toggle this help"));
        lines.push(Line::from("  q, Esc, Ctrl-c    Quit"));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Place and bookmarks are saved automatically.",
            Style::default().fg(Color::DarkGray),
        )));

        let help = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Keyboard Controls ")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: false });

        frame.render_widget(help, popup_area);
    }
}
