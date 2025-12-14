//! Shell completions command

use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, Shell};
use std::io;

use crate::Cli;

#[derive(Args)]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    pub shell: ShellType,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Elvish,
}

impl From<ShellType> for Shell {
    fn from(s: ShellType) -> Self {
        match s {
            ShellType::Bash => Shell::Bash,
            ShellType::Zsh => Shell::Zsh,
            ShellType::Fish => Shell::Fish,
            ShellType::PowerShell => Shell::PowerShell,
            ShellType::Elvish => Shell::Elvish,
        }
    }
}

pub fn run(args: &CompletionsArgs) -> anyhow::Result<()> {
    let mut cmd = Cli::command();
    let shell: Shell = args.shell.into();
    generate(shell, &mut cmd, "parsnip", &mut io::stdout());
    Ok(())
}
