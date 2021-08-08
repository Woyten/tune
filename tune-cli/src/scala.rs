use structopt::StructOpt;

use crate::{
    shared::{KbmOptions, SclCommand},
    App, CliResult,
};

#[derive(StructOpt)]
pub(crate) struct SclOptions {
    /// Name of the scale
    #[structopt(long = "--name")]
    name: Option<String>,

    #[structopt(subcommand)]
    scl: SclCommand,
}

#[derive(StructOpt)]
pub(crate) enum KbmCommand {
    /// Provide a reference note
    #[structopt(name = "ref-note")]
    WithRefNote {
        #[structopt(flatten)]
        kbm: KbmOptions,
    },
}

impl SclOptions {
    pub fn run(self, app: &mut App) -> CliResult<()> {
        Ok(app.write(format_args!("{}", self.scl.to_scl(self.name)?.export()))?)
    }
}

impl KbmCommand {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let KbmCommand::WithRefNote { kbm } = self;
        Ok(app.write(format_args!("{}", kbm.to_kbm()?.export()))?)
    }
}
