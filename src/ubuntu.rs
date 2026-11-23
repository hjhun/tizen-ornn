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
    let helper = root.join("scripts/linux-backend");
    if env::var("SUDO_UID").is_ok_and(|uid| !uid.is_empty() && uid != "0") {
        // Resolve per-user settings after switching back to the invoking user.
        return Err(Command::new(&helper)
            .arg("--as-caller")
            .arg(&exe)
            .args(env::args_os().skip(1))
            .exec());
    }
    let mut req = json!({
        "arch":cli.arch, "headless":cli.headless, "pick":env::args_os().len()==1,
        "directory":null, "command":null
    });
    let proxy = match &cli.command {
        Some(Action::Build { .. } | Action::Update { .. } | Action::Install { .. }) => {
            Some(tizen_ornn::proxy::BuildProxy::load()?)
        }
        _ => None,
    };
    let (name, fields) = match cli.command {
        Some(Action::ShareDir { path, .. }) => {
            return tizen_ornn::workdir::save_share(path.as_deref().unwrap_or(""));
        }
        None => ("run", Value::Null),
        Some(Action::Console { image_dir }) => ("console", json!({"directory":image_dir})),
        Some(Action::Images) => ("images", Value::Null),
        Some(Action::Build { repository, name }) => {
            if let Some(name) = &name {
                crate::build_source::validate_name(name)?;
            }
            let source = repository
                .as_deref()
                .map(|input| {
                    crate::build_source::BuildSource::resolve(input, proxy.as_ref().unwrap())
                })
                .transpose()?;
            let name = name.or_else(|| source.as_ref().map(|s| s.release.clone()));
            (
                "build",
                json!({"name":name, "build_config":source.as_ref().map(|s| s.config())}),
            )
        }
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
        Some(Action::Shutdown { image_dir }) => ("shutdown", json!({"directory":image_dir})),
        Some(Action::Rollback) => ("rollback", Value::Null),
    };
    req["command"] = json!(name);
    if let Value::Object(values) = fields {
        req.as_object_mut().unwrap().extend(values);
    }
    if name == "run" && req["share"].is_null() && req["share_ro"].is_null() {
        req["saved_share"] = json!(tizen_ornn::workdir::shared_directory()?);
        if req["saved_share"].is_null() {
            req["default_share"] = json!(tizen_ornn::workdir::default_share()?);
        }
    }
    let mut backend = Command::new(helper);
    if ["TIZEN_ORNN_HOME", "TIZEN_OUT"]
        .iter()
        .all(|key| env::var_os(key).is_none_or(|value| value.is_empty()))
    {
        backend.env("TIZEN_ORNN_HOME", tizen_ornn::workdir::current()?);
    }
    if matches!(name, "build" | "install" | "update")
        && let Some(path) = tizen_ornn::workdir::ca_setting()?
    {
        backend.env("TIZEN_BUILD_CA_CERT", path);
    }
    if let Some(proxy) = proxy {
        proxy.apply(&mut backend);
    }
    Err(backend.arg(req.to_string()).exec())
}
