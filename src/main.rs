use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};
use tizen_ornn::VERSION;
mod build_source;

#[cfg(target_os = "linux")]
mod ubuntu;
#[cfg(windows)]
mod windows;

#[derive(Parser)]
#[command(name = "tizen-ornn", version = VERSION, about = "Build and run Tizen development images")]
struct Cli {
    /// Open the graphical guest display.
    #[arg(long, global = true, conflicts_with = "headless")]
    gui: bool,
    /// Use the guest serial console only.
    #[arg(long, global = true)]
    headless: bool,
    #[cfg(target_os = "linux")]
    #[arg(long, global = true, value_parser = ["x86_64", "armv7l"])]
    arch: Option<String>,
    #[command(subcommand)]
    command: Option<Action>,
}

#[derive(Subcommand)]
enum Action {
    /// Save the host folder mounted at /home/tizen/share (or clear the setting).
    ShareDir {
        #[arg(required_unless_present = "reset", conflicts_with = "reset")]
        path: Option<String>,
        #[arg(long)]
        reset: bool,
    },
    /// List available images as JSON.
    Images,
    /// Create a development image or reuse the installed disk.
    Build {
        /// Build from a compatible HTTPS repository, snapshot directory, or build.xml.
        #[arg(long)]
        repository: Option<String>,
        /// Save the new image under this name. A URL defaults to its dated release ID.
        #[arg(long)]
        name: Option<String>,
    },
    /// Start the selected image and open its console.
    #[command(visible_alias = "start")]
    Run {
        /// Use the kernel and disk in this directory.
        #[arg(long)]
        image_dir: Option<PathBuf>,
        #[arg(long, conflicts_with = "share_ro")]
        share: Option<PathBuf>,
        #[arg(long)]
        share_ro: Option<PathBuf>,
        #[cfg(target_os = "linux")]
        #[arg(long)]
        host_browser: bool,
        #[cfg(target_os = "linux")]
        #[arg(long)]
        no_host_network: bool,
    },
    #[cfg(target_os = "linux")]
    /// Show or edit the snapshot source.
    Config {
        #[arg(long)]
        repository: Option<String>,
        #[arg(long)]
        snapshot: Option<String>,
    },
    /// Shut down the selected guest cleanly.
    Shutdown {
        #[arg(long)]
        image_dir: Option<PathBuf>,
    },
    #[cfg(target_os = "linux")]
    /// Install packages in the guest.
    Install { packages: Vec<String> },
    #[cfg(target_os = "linux")]
    /// Update the selected image or its packages.
    Update {
        #[arg(long, conflicts_with = "packages")]
        image: bool,
        #[arg(long)]
        packages: bool,
        #[arg(long)]
        snapshot: Option<String>,
        #[arg(requires = "packages")]
        names: Vec<String>,
    },
    #[cfg(target_os = "linux")]
    /// Delete a managed disk.
    Delete {
        image: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    #[cfg(target_os = "linux")]
    /// Select the previous image.
    Rollback,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if (cli.gui || cli.headless) && !matches!(cli.command, None | Some(Action::Run { .. })) {
        eprintln!("--gui and --headless apply only to run/start");
        return ExitCode::from(2);
    }
    #[cfg(windows)]
    let result = windows::execute(cli);
    #[cfg(target_os = "linux")]
    let result = ubuntu::execute(cli);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tizen-ornn: {e}");
            ExitCode::FAILURE
        }
    }
}
