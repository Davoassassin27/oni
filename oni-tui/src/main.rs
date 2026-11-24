use clap::Parser;

use oni_tui::{Options, run};

#[derive(Parser)]
#[command(
    name = "oni-tui",
    version,
    about = "ONI terminal UI — supervise agent tasks"
)]
struct Args {
    /// Daemon base URL
    #[arg(long, env = "ONI_URL", default_value = "http://127.0.0.1:7766")]
    url: String,

    /// Bearer token when the daemon runs with ONI_TOKEN
    #[arg(long, env = "ONI_TOKEN")]
    token: Option<String>,

    /// Refresh interval in seconds
    #[arg(long, default_value_t = 2)]
    interval: u64,

    /// Skip the TerminalTextEffects intro
    #[arg(long)]
    no_intro: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(error) = run(Options {
        url: args.url,
        token: args.token,
        interval: args.interval,
        no_intro: args.no_intro,
    }) {
        eprintln!("oni-tui: {error:#}");
        std::process::exit(1);
    }
}
