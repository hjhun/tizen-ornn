#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "desktop/design.rs"]
mod design;
#[path = "desktop/tray.rs"]
mod tray;
use design::{INK, MUTED};
use eframe::egui::{self, Color32, RichText};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tizen_ornn::{Image, sibling};

fn cli() -> Result<Command, String> {
    let path = sibling("tizen-ornn").map_err(|e| e.to_string())?;
    if !path.is_file() {
        return Err(format!("Missing CLI: {}", path.display()));
    }
    let cmd = Command::new(path);
    #[cfg(windows)]
    let cmd = {
        let mut cmd = cmd;
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW for image discovery.
        cmd
    };
    Ok(cmd)
}

fn guest_arch(path: &std::path::Path) -> Option<&'static str> {
    if path.join("bzImage").is_file()
        && ["tizen-rootfs.ext4", "tizen-rootfs.qcow2"]
            .iter()
            .any(|file| path.join(file).is_file())
    {
        return Some("x86_64");
    }
    tizen_ornn::arm_arch(path)
}

fn connected_directories(store: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    match std::fs::read(store.join("connected-images.json")) {
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn update_connection(
    store: &std::path::Path,
    path: &std::path::Path,
    add: bool,
) -> std::io::Result<()> {
    std::fs::create_dir_all(store)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.join(".connected-images.lock"))?;
    lock.lock()?;
    let mut paths = connected_directories(store)?;
    if paths.iter().any(|entry| entry == path) != add {
        if add {
            paths.push(path.to_owned());
        } else {
            paths.retain(|entry| entry != path);
        }
        let mut tmp = tempfile::NamedTempFile::new_in(store)?;
        serde_json::to_writer(&mut tmp, &paths)?;
        tmp.persist(store.join("connected-images.json"))?;
    }
    Ok(())
}

#[derive(Clone)]
struct Removal {
    store: PathBuf,
    image: PathBuf,
    external: bool,
}

#[derive(Default)]
struct Launcher {
    images: Vec<Image>,
    external: Vec<PathBuf>,
    removal: Option<Removal>,
    selected: Option<PathBuf>,
    stopping: Option<(PathBuf, Child)>,
    folder_dialog: Option<egui_file_dialog::FileDialog>,
    name: String,
    repository: String,
    build_open: bool,
    build_arch: Option<&'static str>,
    workdir: String,
    share: String,
    active_workdir: String,
    settings_open: bool,
    status: String,
    processes: Vec<Child>,
    monitored: std::collections::HashSet<PathBuf>,
    monitors: Vec<Child>,
    last_tray_check: Option<std::time::Instant>,
}

impl Launcher {
    fn attach_tray(&mut self, dir: &std::path::Path) {
        if !self.monitored.contains(dir) {
            match tray::start(dir) {
                Ok(child) => {
                    self.monitors.push(child);
                    self.monitored.insert(dir.to_path_buf());
                }
                Err(e) => self.status = format!("Could not start notification icon: {e}"),
            }
        }
    }

    fn update_trays(&mut self) {
        if self
            .last_tray_check
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.last_tray_check = Some(std::time::Instant::now());
        self.monitors.retain_mut(|child| match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                self.status = "Desktop notification service unavailable.".into();
                false
            }
            Ok(None) => true,
            _ => false,
        });
        for dir in self
            .images
            .iter()
            .map(|i| i.path.clone())
            .collect::<Vec<_>>()
        {
            match tray::running(&dir) {
                Ok(true) => self.attach_tray(&dir),
                Ok(false) => {
                    self.monitored.remove(&dir);
                }
                Err(e) => self.status = e.to_string(),
            }
        }
    }

    fn configure(&self, command: &mut Command) -> Result<(), String> {
        let dir = tizen_ornn::workdir::current().map_err(|e| e.to_string())?;
        command.env("TIZEN_ORNN_HOME", dir).env_remove("TIZEN_OUT");
        Ok(())
    }

    fn refresh(&mut self) -> Result<(), String> {
        self.active_workdir = tizen_ornn::workdir::current()
            .map_err(|e| e.to_string())?
            .display()
            .to_string();
        let mut command = cli()?;
        self.configure(&mut command)?;
        let output = command.arg("images").output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        self.images = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        self.external.clear();
        for path in connected_directories(std::path::Path::new(&self.active_workdir))
            .map_err(|e| e.to_string())?
        {
            if !self.images.iter().any(|image| image.path == path)
                && let Some(arch) = guest_arch(&path)
            {
                self.external.push(path.clone());
                self.images.push(Image {
                    path,
                    arch: arch.into(),
                });
            }
        }
        if !self
            .images
            .iter()
            .any(|i| Some(&i.path) == self.selected.as_ref())
        {
            self.selected = self.images.first().map(|i| i.path.clone());
        }
        Ok(())
    }

    fn connect_directory(&mut self, path: PathBuf) -> std::io::Result<()> {
        let arch = guest_arch(&path).ok_or_else(|| {
            std::io::Error::other("Choose an image directory containing a guest kernel and disk.")
        })?;
        if !self.images.iter().any(|image| image.path == path) {
            let store = std::path::Path::new(&self.active_workdir);
            update_connection(store, &path, true)?;
            self.external.push(path.clone());
            self.images.push(Image {
                path: path.clone(),
                arch: arch.into(),
            });
        }
        self.selected = Some(path);

        Ok(())
    }

    fn terminal(&mut self, args: &[&str]) -> Result<(), String> {
        let path = sibling("tizen-ornn").map_err(|e| e.to_string())?;
        #[cfg(windows)]
        let mut cmd = {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new(path);
            c.creation_flags(0x10); // CREATE_NEW_CONSOLE
            c
        };
        #[cfg(target_os = "linux")]
        let mut cmd = {
            let mut c = Command::new("x-terminal-emulator");
            // Minimal Ubuntu installations can have Zutty without its default X11 bitmap font.
            let terminal = std::env::var_os("PATH").and_then(|paths| {
                std::env::split_paths(&paths)
                    .find_map(|dir| dir.join("x-terminal-emulator").canonicalize().ok())
            });
            if terminal
                .as_ref()
                .and_then(|p| p.file_name())
                .is_some_and(|name| name == "zutty")
                && !std::path::Path::new("/usr/share/fonts/X11/misc/9x18.pcf.gz").is_file()
                && std::path::Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")
                    .is_file()
            {
                c.args(["-font", "DejaVuSansMono"]);
            }
            c.arg("-e").arg(path);
            c
        };
        self.configure(&mut cmd)?;
        let child = cmd
            .args(args)
            .stdin(Stdio::inherit())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.processes.push(child);
        Ok(())
    }

    fn build_form(&mut self, ui: &mut egui::Ui) {
        ui.label("Guest architecture");
        self.build_arch.get_or_insert("x86_64");
        ui.horizontal_wrapped(|ui| {
            for arch in ["x86_64", "armv7l", "aarch64"] {
                ui.selectable_value(&mut self.build_arch, Some(arch), arch);
            }
        });
        let arch = self.build_arch.unwrap_or("x86_64");
        let arm = arch != "x86_64";
        if arm {
            ui.label(format!(
                "Official Tizen 10.1 headed {} + generic HAL · tizen-10.1-unified_20261119.053314",
                arch
            ));
            ui.label("Graphical boot with a serial terminal. The kernel version comes from the matching ARM boot image’s .packages file. This build uses the configured snapshot and the image name shown above. Custom RPM repositories, local archives, host folders and package updates are unsupported.");
            if cfg!(windows) {
                ui.label(
                    "Windows uses a disposable Ubuntu builder VM and native QEMU for the guest.",
                );
            } else {
                ui.label(
                    "Install qemu-system-arm and the matching cross compiler before building.",
                );
            }
        } else {
            ui.label("The builder downloads the official source archive and development RPMs. Use an HTTPS snapshot or compatible mirror. Additional RPM repository URLs are unsupported. For a local archive, use scripts/tizen-image --archive on Ubuntu.");
            ui.label("Snapshot repository URL");
            ui.add(
                egui::TextEdit::singleline(&mut self.repository)
                    .desired_width(f32::INFINITY)
                    .hint_text("https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/"),
            );
            ui.label("Image name");
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .desired_width(f32::INFINITY)
                    .hint_text("Leave blank to use the snapshot ID, including dots"),
            );
        }
        if ui.add(design::action("Build")).clicked() {
            let repository = self.repository.trim().to_owned();
            let name = self.name.trim().to_owned();
            let mut args = vec!["build", "--arch", arch];
            if !arm && !repository.is_empty() {
                args.extend(["--repository", &repository]);
            }
            if !arm && !name.is_empty() {
                args.extend(["--name", &name]);
            }
            match self.terminal(&args) {
                Ok(()) => self.build_open = false,
                Err(e) => self.status = e,
            }
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(design::heading("Work directory", 20.0));
            ui.label(
                RichText::new("Images, build files and downloads are stored here.").color(MUTED),
            );
            let default = tizen_ornn::workdir::default_dir().unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut self.workdir)
                    .margin(egui::Margin::symmetric(10, 7))
                    .hint_text(default.display().to_string())
                    .desired_width(f32::INFINITY),
            );
            ui.label(
                RichText::new(format!("Current folder: {}", self.active_workdir)).color(MUTED),
            );
            if ui.add(design::action("Save work directory")).clicked() {
                self.status = match tizen_ornn::workdir::save(&self.workdir) {
                    Ok(()) => self
                        .refresh()
                        .err()
                        .unwrap_or_else(|| "Work directory saved.".into()),
                    Err(e) => format!("Could not save work directory: {e}"),
                };
            }
            ui.separator();
            ui.label(design::heading("Shared folder (x86_64)", 20.0));
            ui.label("Available at /home/tizen/share and /mnt/host when the image starts.");
            ui.add(
                egui::TextEdit::singleline(&mut self.share)
                    .desired_width(f32::INFINITY)
                    .hint_text(
                        PathBuf::from(&self.active_workdir)
                            .join("share")
                            .display()
                            .to_string(),
                    ),
            );
            if ui.add(design::action("Save shared folder")).clicked() {
                self.status = match tizen_ornn::workdir::save_share(&self.share) {
                    Ok(()) => "Shared folder saved for the next boot.".into(),
                    Err(error) => error.to_string(),
                };
            }
        });
    }

    fn library(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        let wide = ui.available_width() >= 740.0;
        if wide {
            ui.horizontal(|ui| {
                ui.label(design::heading("Image library", 30.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.library_actions(ui)
                });
            });
        } else {
            ui.label(design::heading("Image library", 26.0));
            ui.horizontal_wrapped(|ui| self.library_actions(ui));
        }
        ui.add_space(8.0);
        if self.images.is_empty() {
            design::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(design::heading("No images yet", 20.0));
                ui.label("Open a folder containing a guest image or build one from a snapshot.");
            });
            return Ok(());
        }
        if wide {
            let sidebar = (ui.available_width() * 0.32).clamp(236.0, 370.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar, ui.ctx().screen_rect().height() * 0.65),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.image_list(ui),
                );
                ui.add_space(6.0);
                ui.vertical(|ui| self.image_details(ui, true)).inner
            })
            .inner
        } else {
            let selected = self
                .selected
                .as_deref()
                .and_then(|dir| dir.file_name())
                .unwrap_or_default()
                .to_string_lossy();
            egui::ComboBox::from_id_salt("compact-image-picker")
                .width(ui.available_width())
                .truncate()
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for image in &self.images {
                        let name = image.path.file_name().unwrap_or_default().to_string_lossy();
                        ui.selectable_value(
                            &mut self.selected,
                            Some(image.path.clone()),
                            name.as_ref(),
                        )
                        .on_hover_text(image.path.display().to_string());
                    }
                });
            ui.add_space(6.0);
            self.image_details(ui, false)
        }
    }

    fn library_actions(&mut self, ui: &mut egui::Ui) {
        let open_button = if self.images.is_empty() {
            design::action("Add image…")
        } else {
            design::secondary("Add image…")
        };
        let open = ui.add(open_button).clicked();
        let build = ui.add(design::secondary("Build image…")).clicked();
        if ui.add(egui::Button::new("Refresh").frame(false)).clicked() {
            self.status = self.refresh().err().unwrap_or_default();
        }
        if build {
            open_popup(ui.ctx(), "Build image", &mut self.build_open);
        }
        if open {
            if self.folder_dialog.is_some() {
                ui.ctx().send_viewport_cmd_to(
                    popup_id("Open image folder"),
                    egui::ViewportCommand::Focus,
                );
            } else {
                let mut dialog = egui_file_dialog::FileDialog::new()
                    .title_bar(false)
                    .fixed_pos(egui::Pos2::ZERO)
                    .resizable(false);
                dialog.pick_directory();
                self.folder_dialog = Some(dialog);
            }
        }
    }

    fn image_list(&mut self, ui: &mut egui::Ui) {
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(design::heading(
                format!("Images ({})", self.images.len()),
                16.0,
            ));
            ui.separator();
            let height = ui.ctx().screen_rect().height() * 0.55;
            egui::ScrollArea::vertical()
                .id_salt("image-list")
                .max_height(height)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    for image in &self.images {
                        let selected = self.selected.as_ref() == Some(&image.path);
                        let name = image.path.file_name().unwrap_or_default().to_string_lossy();
                        let button = egui::Button::new(RichText::new(name).color(if selected {
                            design::ACCENT
                        } else {
                            INK
                        }))
                        .fill(if selected {
                            Color32::from_rgb(231, 239, 251)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(6)
                        .truncate()
                        .min_size(egui::vec2(ui.available_width(), 38.0));
                        if ui
                            .add(button)
                            .on_hover_text(image.path.display().to_string())
                            .clicked()
                        {
                            self.selected = Some(image.path.clone());
                        }
                    }
                });
        });
    }

    fn image_details(&mut self, ui: &mut egui::Ui, show_name: bool) -> Result<(), String> {
        let Some(dir) = self.selected.clone() else {
            return Ok(());
        };
        let arch = guest_arch(&dir).ok_or("Image files are missing.")?;
        let arm = arch != "x86_64";
        let running = tray::running(&dir).map_err(|e| e.to_string())?;
        let stopping = self.stopping.as_ref().is_some_and(|(path, _)| path == &dir);
        let path = dir.to_str().ok_or("Invalid image directory")?;
        design::card()
            .show(ui, |ui| -> Result<(), String> {
                ui.set_width(ui.available_width());
                if show_name {
                    ui.label(design::heading(
                        dir.file_name().unwrap_or_default().to_string_lossy(),
                        23.0,
                    ));
                }
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                    let (text, color) = if stopping {
                        ("Shutting down…", Color32::from_rgb(169, 111, 25))
                    } else if running {
                        ("Running", Color32::from_rgb(32, 129, 92))
                    } else {
                        ("Stopped", MUTED)
                    };
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                    ui.label(RichText::new(text).size(14.0).color(MUTED));
                });
                ui.horizontal_wrapped(|ui| -> Result<(), String> {
                    if running {
                        if ui
                            .add_enabled(!stopping, design::action("Open terminal"))
                            .clicked()
                        {
                            self.terminal(&["console", "--image-dir", path])?;
                            self.status.clear();
                        }
                    } else {
                        if ui
                            .add_enabled(!stopping, design::action("Start with display"))
                            .clicked()
                        {
                            let mut args = vec!["run", "--gui", "--image-dir", path];
                            if arm {
                                args.extend(["--arch", arch]);
                            }
                            self.terminal(&args)?;
                            self.attach_tray(&dir);
                            self.status.clear();
                        }
                        if !arm
                            && ui
                                .add_enabled(!stopping, design::secondary("Start terminal only"))
                                .clicked()
                        {
                            self.terminal(&["run", "--headless", "--image-dir", path])?;
                            self.attach_tray(&dir);
                            self.status.clear();
                        }
                    }
                    if running
                        && ui
                            .add_enabled(self.stopping.is_none(), design::secondary("Shut down"))
                            .clicked()
                    {
                        let mut cmd = cli()?;
                        self.configure(&mut cmd)?;
                        let child = cmd
                            .args(["shutdown", "--image-dir", path])
                            .stderr(Stdio::piped())
                            .spawn()
                            .map_err(|e| e.to_string())?;
                        self.stopping = Some((dir.clone(), child));
                        self.status.clear();
                    }
                    Ok(())
                })
                .inner?;
                ui.add_space(2.0);
                ui.separator();
                ui.label(RichText::new("Image folder").color(MUTED).size(13.0));
                ui.add(egui::Label::new(path).wrap());
                ui.label(RichText::new("Architecture").color(MUTED).size(13.0));
                ui.label(arch);
                let store = PathBuf::from(&self.active_workdir);
                let external = self.external.contains(&dir);
                let expected_parent = if arm && cfg!(target_os = "linux") {
                    store.clone()
                } else {
                    store.join("images")
                };
                let managed = dir
                    .parent()
                    .and_then(|p| p.canonicalize().ok())
                    .is_some_and(|parent| Some(parent) == expected_parent.canonicalize().ok())
                    && (!arm || dir.ends_with(format!("{arch}-headed")));
                if (external || managed)
                    && ui
                        .add_enabled(
                            !running && !stopping,
                            design::secondary(if external {
                                "Disconnect image…"
                            } else {
                                "Delete image…"
                            }),
                        )
                        .clicked()
                {
                    self.removal = Some(Removal {
                        store,
                        image: dir.clone(),
                        external,
                    });
                }
                Ok(())
            })
            .inner?;
        Ok(())
    }
}

fn popup_id(title: &str) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(title)
}

fn open_popup(ctx: &egui::Context, title: &str, open: &mut bool) {
    if *open {
        ctx.send_viewport_cmd_to(popup_id(title), egui::ViewportCommand::Focus);
    }
    *open = true;
}

fn popup(
    ctx: &egui::Context,
    title: &str,
    size: [f32; 2],
    minimum: [f32; 2],
    mut draw: impl FnMut(&egui::Context),
) -> bool {
    ctx.show_viewport_immediate(
        popup_id(title),
        egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(size)
            .with_min_inner_size(minimum)
            .with_resizable(true),
        |child, _| {
            draw(child);
            child.input(|i| i.viewport().close_requested())
        },
    )
}

struct Desktop {
    logo: egui::TextureHandle,
    launcher: Launcher,
}

impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let app = &mut self.launcher;
        app.update_trays();
        let mut finished = false;
        app.processes.retain_mut(|p| match p.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                if !status.success() {
                    app.status = format!("Terminal exited with {status}");
                }
                finished = true;
                false
            }
            Err(e) => {
                app.status = e.to_string();
                false
            }
        });
        if finished && let Err(e) = app.refresh() {
            app.status = e;
        }
        if let Some((_, proc)) = app.stopping.as_mut() {
            match proc.try_wait() {
                Ok(Some(_)) => {
                    let (_, child) = app.stopping.take().unwrap();
                    app.status = match child.wait_with_output() {
                        Ok(out) if out.status.success() => "Guest shut down.".into(),
                        Ok(out) => String::from_utf8_lossy(&out.stderr).trim().to_owned(),
                        Err(e) => e.to_string(),
                    };
                }
                Err(e) => {
                    app.stopping = None;
                    app.status = e.to_string();
                }
                Ok(None) => (),
            }
        }
        if app.selected.is_some() || !app.processes.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(28, 41, 60))
                    .inner_margin(egui::Margin::symmetric(22, 9)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.image((self.logo.id(), egui::vec2(36.0, 36.0)));
                    ui.label(design::heading("Tizen Ornn", 19.0).color(Color32::WHITE));
                    ui.label(
                        RichText::new(tizen_ornn::VERSION)
                            .size(13.0)
                            .color(Color32::from_rgb(196, 208, 223)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Settings").color(Color32::WHITE))
                                    .fill(Color32::from_rgb(51, 66, 87))
                                    .corner_radius(7),
                            )
                            .clicked()
                        {
                            open_popup(ctx, "Settings", &mut app.settings_open);
                        }
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(design::PAPER)
                    .inner_margin(egui::Margin::symmetric(24, 18)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("page")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Err(error) = app.library(ui) {
                            app.status = error;
                        }
                        if !app.status.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new(&app.status).color(INK));
                        }
                    });
            });
        if let Some(removal) = app.removal.clone() {
            let title = if removal.external {
                "Disconnect image"
            } else {
                "Delete image"
            };
            let mut confirm = false;
            let mut cancel = false;
            let closed = popup(ctx, title, [460.0, 205.0], [320.0, 170.0], |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(design::heading(title, 22.0));
                        ui.label(if removal.external {
                            "Remove this folder from the library and keep its files."
                        } else {
                            "Permanently delete the guest disk and all data stored inside it?"
                        });
                        ui.add(egui::Label::new(removal.image.display().to_string()).wrap());
                        ui.horizontal(|ui| {
                            cancel = ui.add(design::secondary("Cancel")).clicked();
                            confirm = ui
                                .add(design::secondary(if removal.external {
                                    "Disconnect"
                                } else {
                                    "Delete disk"
                                }))
                                .clicked();
                        });
                    });
                });
            });
            if closed || cancel || confirm {
                app.removal = None;
            }
            if confirm && !closed {
                app.status = (|| -> Result<String, String> {
                    if removal.external {
                        update_connection(&removal.store, &removal.image, false)
                            .map_err(|e| e.to_string())?;
                    } else {
                        let name = removal.image.file_name().ok_or("Missing image name")?;
                        let mut cmd = cli()?;
                        cmd.env("TIZEN_ORNN_HOME", &removal.store)
                            .env_remove("TIZEN_OUT");
                        #[cfg(target_os = "linux")]
                        cmd.args([
                            "--arch",
                            guest_arch(&removal.image)
                                .ok_or("Image has changed. Refresh the library.")?,
                        ]);
                        let out = cmd
                            .args(["delete", "--yes", "--"])
                            .arg(name)
                            .output()
                            .map_err(|e| e.to_string())?;
                        if !out.status.success() {
                            return Err(String::from_utf8_lossy(&out.stderr).trim().into());
                        }
                    }
                    app.refresh()?;
                    Ok(if removal.external {
                        "Image disconnected."
                    } else {
                        "Guest disk deleted."
                    }
                    .into())
                })()
                .unwrap_or_else(|error| error);
            }
        }
        if app.settings_open {
            let closed = popup(ctx, "Settings", [560.0, 420.0], [380.0, 250.0], |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        app.settings(ui);
                        if !app.status.is_empty() {
                            ui.label(&app.status);
                        }
                    });
                });
            });
            app.settings_open = !closed;
        }
        if app.build_open {
            let closed = popup(ctx, "Build image", [550.0, 390.0], [320.0, 180.0], |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        app.build_form(ui);
                    });
                });
            });
            app.build_open &= !closed;
        }
        if let Some(dialog) = &mut app.folder_dialog {
            let closed = popup(
                ctx,
                "Open image folder",
                [780.0, 540.0],
                [540.0, 350.0],
                |ctx| {
                    let frame = egui::Frame::window(&ctx.style());
                    let size = ctx.screen_rect().size() - frame.total_margin().sum();
                    dialog.config_mut().min_size = size;
                    dialog.config_mut().max_size = Some(size);
                    dialog.update(ctx);
                },
            );
            if let Some(path) = dialog.take_picked() {
                app.folder_dialog = None;
                app.status = app
                    .connect_directory(path)
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_default();
            } else if closed || dialog.state() != egui_file_dialog::DialogState::Open {
                app.folder_dialog = None;
            }
        }
    }
}

fn main() -> eframe::Result {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next();
    let image = args.next().map(PathBuf::from);
    if mode.as_deref() == Some(std::ffi::OsStr::new("--tray-monitor")) {
        if image
            .as_deref()
            .is_none_or(|dir| tray::monitor(dir).is_err())
        {
            std::process::exit(1);
        }
        return Ok(());
    }
    let mut app = Launcher::default();
    if let Ok(Some(path)) = tizen_ornn::workdir::setting() {
        app.workdir = path.display().to_string();
    }
    if let Ok(Some(path)) = tizen_ornn::workdir::shared_directory() {
        app.share = path.display().to_string();
    }
    if let Err(e) = app.refresh() {
        app.status = e;
    }
    if mode.as_deref() == Some(std::ffi::OsStr::new("--settings")) {
        app.settings_open = true;
        if let Some(dir) = image
            && app.images.iter().any(|i| i.path == dir)
        {
            app.selected = Some(dir);
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(
                if std::env::var_os("TIZEN_ORNN_PORTABLE_LAUNCHER").is_some() {
                    "tizen-ornn-portable"
                } else {
                    "tizen-ornn"
                },
            )
            .with_icon(eframe::icon_data::from_png_bytes(design::ICON).expect("embedded app icon"))
            .with_inner_size([960.0, 580.0])
            .with_min_inner_size([380.0, 320.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tizen Ornn",
        options,
        Box::new(move |cc| {
            design::configure(&cc.egui_ctx);
            let icon = eframe::icon_data::from_png_bytes(design::ICON).expect("embedded icon");
            let logo = cc.egui_ctx.load_texture(
                "ornn-logo",
                egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                ),
                egui::TextureOptions::LINEAR,
            );
            Ok(Box::new(Desktop {
                launcher: app,
                logo,
            }))
        }),
    )
}
