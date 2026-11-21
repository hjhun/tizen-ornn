use super::{Action, Cli};
use serde_json::{Value, json};
use std::{env, io, os::unix::process::CommandExt, process::Command};

pub fn execute(cli: Cli) -> io::Result<()> {
    let exe = env::current_exe()?;
    let parent = exe.parent().unwrap();
    let root = if parent.join("scripts/linux-backend").is_file() {
        parent
    } else {
        parent
            .ancestors()
            .find(|p| p.join("Cargo.toml").is_file() && p.join("scripts/linux-backend").is_file())
            .ok_or_else(|| {
                io::Error::other("Install the Ubuntu package with its runtime helpers.")
            })?
    };
    let mut req = json!({
        "arch":cli.arch, "headless":cli.headless, "pick":env::args_os().len()==1,
        "directory":null, "command":null
    });
    let (name, fields) = match cli.command {
        None => ("run", Value::Null),
        Some(Action::Images) => ("images", Value::Null),
        Some(Action::Build {}) => ("build", Value::Null),
        Some(Action::Run {
            image_dir,
            share,
            share_ro,
            host_browser,
            no_host_network,
        }) => (
            "run",
            json!({"directory":image_dir,"share":share,"share_ro":share_ro,
                         "host_browser":host_browser,"no_host_network":no_host_network}),
        ),
        Some(Action::Config {
            repository,
            snapshot,
        }) => (
            "config",
            json!({ "repository":repository,"snapshot":snapshot }),
        ),
        Some(Action::Delete { image, yes }) => ("delete", json!({"image":image,"yes":yes})),
        Some(Action::Install { packages }) => ("install", json!({"packages":packages})),
        Some(Action::Update {
            packages,
            snapshot,
            names,
            ..
        }) => (
            "update",
            json!({"packages":packages,"snapshot":snapshot,"names":names}),
        ),
        Some(Action::Shutdown) => ("shutdown", Value::Null),
        Some(Action::Rollback) => ("rollback", Value::Null),
    };
    req["command"] = json!(name);
    if let Value::Object(values) = fields {
        req.as_object_mut().unwrap().extend(values);
    }
    Err(Command::new(root.join("scripts/linux-backend"))
        .arg(req.to_string())
        .exec())
}
