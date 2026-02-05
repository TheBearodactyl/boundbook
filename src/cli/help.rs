use {
    clap::CommandFactory,
    clap_help::Printer,
    termimad::{CompoundStyle, ansi},
};

#[allow(dead_code)]
pub enum RosePineVariant {
    Main,
    Moon,
    Dawn,
}

static INTRO: &str = "

Use, create, and modify BBF (Bound Book Format) archives
";

#[macroni_n_cheese::mathinator2000]
pub fn rose_pine_printer<'a>(variant: RosePineVariant, intro: Option<&'a str>) -> Printer<'a> {
    let mut printer: Printer<'a> = Printer::new(super::Cli::command())
        .without("author")
        .with("introduction", INTRO)
        .with("options", clap_help::TEMPLATE_OPTIONS);

    if let Some(intro_text) = intro {
        printer = printer.with("introduction", intro_text);
    }

    printer.template_keys_mut().push("examples");
    printer.set_template("examples", EXAMPLES_TEMPLATE);
    for (i, example) in EXAMPLES.iter().enumerate() {
        printer
            .expander_mut()
            .sub("examples")
            .set("example-number", i + 1)
            .set("example-title", example.title)
            .set("example-cmd", example.cmd)
            .set_md("example-comments", example.comments);
    }

    let skin = printer.skin_mut();

    match variant {
        RosePineVariant::Main => {
            skin.headers[0].compound_style.set_fg(ansi(183));
            skin.bold.set_fg(ansi(181));
            skin.italic = CompoundStyle::with_fg(ansi(152));
            skin.inline_code = CompoundStyle::with_fg(ansi(222));
            skin.table.compound_style.set_fg(ansi(103));
        }
        RosePineVariant::Moon => {
            skin.headers[0].compound_style.set_fg(ansi(183));
            skin.bold.set_fg(ansi(174));
            skin.italic = CompoundStyle::with_fg(ansi(152));
            skin.inline_code = CompoundStyle::with_fg(ansi(222));
            skin.table.compound_style.set_fg(ansi(103));
        }
        RosePineVariant::Dawn => {
            skin.headers[0].compound_style.set_fg(ansi(133));
            skin.bold.set_fg(ansi(173));
            skin.italic = CompoundStyle::with_fg(ansi(73));
            skin.inline_code = CompoundStyle::with_fg(ansi(172));
            skin.table.compound_style.set_fg(ansi(103));
        }
    }

    skin.table_border_chars = termimad::ROUNDED_TABLE_BORDER_CHARS;

    printer
}

pub fn rose_pine_printer_for_subcommand<'a>(
    subcommand_name: &str,
    variant: RosePineVariant,
) -> Option<Printer<'a>> {
    let cmd = super::Cli::command();

    cmd.find_subcommand(subcommand_name).map(|subcmd| {
        let mut printer = Printer::new(subcmd.clone());

        let skin = printer.skin_mut();

        match variant {
            RosePineVariant::Main => {
                skin.headers[0].compound_style.set_fg(ansi(183));
                skin.bold.set_fg(ansi(181));
                skin.italic = CompoundStyle::with_fg(ansi(152));
                skin.inline_code = CompoundStyle::with_fg(ansi(222));
                skin.table.compound_style.set_fg(ansi(103));
            }
            RosePineVariant::Moon => {
                skin.headers[0].compound_style.set_fg(ansi(183));
                skin.bold.set_fg(ansi(174));
                skin.italic = CompoundStyle::with_fg(ansi(152));
                skin.inline_code = CompoundStyle::with_fg(ansi(222));
                skin.table.compound_style.set_fg(ansi(103));
            }
            RosePineVariant::Dawn => {
                skin.headers[0].compound_style.set_fg(ansi(133));
                skin.bold.set_fg(ansi(173));
                skin.italic = CompoundStyle::with_fg(ansi(73));
                skin.inline_code = CompoundStyle::with_fg(ansi(172));
                skin.table.compound_style.set_fg(ansi(103));
            }
        }

        skin.table_border_chars = termimad::ROUNDED_TABLE_BORDER_CHARS;

        printer
    })
}

pub static EXAMPLES_TEMPLATE: &str = "
**Examples:**

${examples
**${example-number})** ${example-title}: `${example-cmd}`
${example-comments}
}
";

pub struct Example {
    pub title: &'static str,
    pub cmd: &'static str,
    pub comments: &'static str,
}

impl Example {
    pub const fn new(title: &'static str, cmd: &'static str, comments: &'static str) -> Self {
        Self {
            title,
            cmd,
            comments,
        }
    }
}

pub static EXAMPLES: &[Example] = &[
    Example::new(
        "Create a new book from images",
        "boundbook create *.png -o book.bbf",
        "Makes a BBF archive from all PNG files in the current directory and stores it at book.bbf",
    ),
    Example::new(
        "Read a BBF file (with prerendering)",
        "boundbook read --prerender book.bbf",
        "Opens book.bbf, converts all the page images to sixel strings, and displays them in a reader UI",
    ),
    Example::new(
        "Read a BBF file (without prerendering)",
        "boundbook read book.bbf",
        "Opens book.bbf and displays the pages in a reader UI",
    ),
];
