use {
    boundbook::Result,
    clap::{Args, Command, CommandFactory, ValueEnum},
    clap_complete::Generator,
};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, ValueEnum, Clone)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Nushell,
    Clink,
    Fig,
}

impl Generator for Shell {
    fn file_name(&self, name: &str) -> String {
        match self {
            Shell::Bash => format!("{}.bash", name),
            Shell::Elvish => format!("{}.elv", name),
            Shell::Fish => format!("{}.fish", name),
            Shell::PowerShell => format!("_{}.ps1", name),
            Shell::Zsh => format!("_{}", name),
            Shell::Nushell => clap_complete_nushell::Nushell.file_name(name),
            Shell::Clink => clap_complete_clink::Clink.file_name(name),
            Shell::Fig => clap_complete_fig::Fig.file_name(name),
        }
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn std::io::Write) {
        match self {
            Shell::Bash => clap_complete::shells::Bash.generate(cmd, buf),
            Shell::Elvish => clap_complete::shells::Elvish.generate(cmd, buf),
            Shell::Fish => clap_complete::shells::Fish.generate(cmd, buf),
            Shell::PowerShell => clap_complete::shells::PowerShell.generate(cmd, buf),
            Shell::Zsh => clap_complete::shells::Zsh.generate(cmd, buf),
            Shell::Nushell => clap_complete_nushell::Nushell.generate(cmd, buf),
            Shell::Clink => clap_complete_clink::Clink.generate(cmd, buf),
            Shell::Fig => clap_complete_fig::Fig.generate(cmd, buf),
        }
    }
}

#[derive(Args)]
#[command(author = "The Motherfucking Bearodactyl")]
pub struct CompleteArgs {
    shell: Shell,
}

pub fn execute(args: CompleteArgs) -> Result<()> {
    let mut app = crate::cli::Cli::command();
    let bin_name = app.get_name().to_string();
    clap_complete::generate(args.shell, &mut app, bin_name, &mut std::io::stdout());

    Ok(())
}
