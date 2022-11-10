use clap::{Parser, Subcommand, ArgAction};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init {
        #[arg(short, long, action = ArgAction::SetTrue)]
        interactive: Option<bool>
    },
    Edit,
    Plan,
    Destroy,
}
