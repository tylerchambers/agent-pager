use anyhow::{Result, bail};
use clap::Parser;

use agent_pager::{cli::Cli, command::AppCommand, composition::App};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let command = AppCommand::try_from(cli)?;
    let app = App::production()?;
    let outcome = app.run(command).await?;

    for line in outcome.status_lines() {
        println!("{line}");
    }

    if let Some(message) = outcome.failure_message() {
        bail!(message);
    }

    Ok(())
}
