use clap::Parser;

use crate::{
    shared::{KbmOptions, SclCommand},
    App, CliResult,
};

#[derive(Parser)]
pub(crate) struct SclOptions {
    /// Name of the scale
    #[arg(long = "name")]
    name: Option<String>,

    #[command(subcommand)]
    scl: SclCommand,
}

#[derive(Parser)]
pub(crate) enum KbmCommand {
    /// Provide a reference note
    #[command(name = "ref-note")]
    WithRefNote {
        #[command(flatten)]
        kbm: KbmOptions,
    },
}

impl SclOptions {
    pub fn run(self, app: &mut App) -> CliResult {
        Ok(app.write(format_args!("{}", self.scl.to_scl(self.name)?.export()))?)
    }
}

impl KbmCommand {
    pub fn run(&self, app: &mut App) -> CliResult {
        let KbmCommand::WithRefNote { kbm } = self;
        Ok(app.write(format_args!("{}", kbm.to_kbm()?.export()))?)
    }
}
