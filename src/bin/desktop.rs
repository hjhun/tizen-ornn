#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "desktop/design.rs"]
mod design;
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
    #[cfg(target_os = "linux")]
    if path.join("zImage").is_file()
        && ["rootfs.img", "system-data.img", "user.img", "hal.img"]
            .iter()
            .all(|file| path.join("disk").join(file).is_file())
    {
        return Some("armv7l");
    }
    None
}

fn connected_directories(store: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    match std::fs::read(store.join("connected-images.json")) {
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

#[derive(Default)]
struct Launcher {
    images: Vec<Image>,
    selected: Option<PathBuf>,
    stopping: Option<(PathBuf, Child)>,
    folder_dialog: Option<egui_file_dialog::FileDialog>,
    name: String,
    repository: String,
    build_open: bool,
    workdir: String,
    share: String,
    active_workdir: String,
    settings_open: bool,
    status: String,
    processes: Vec<Child>,
}

impl Launcher {
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
        for path in connected_directories(std::path::Path::new(&self.active_workdir))
            .map_err(|e| e.to_string())?
        {
            if !self.images.iter().any(|image| image.path == path)
                && let Some(arch) = guest_arch(&path)
            {
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
            std::fs::create_dir_all(store)?;
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(store.join(".connected-images.lock"))?;
            lock.lock()?;
            let mut paths = connected_directories(store)?;
            if !paths.contains(&path) {
                paths.push(path.clone());
                let mut tmp = tempfile::NamedTempFile::new_in(store)?;
                serde_json::to_writer(&mut tmp, &paths)?;
                tmp.persist(store.join("connected-images.json"))?;
            }
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

    fn settings(&mut self, ui: &mut egui::Ui) {
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Work directory").size(20.0).strong());
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
            ui.label(RichText::new("Shared folder (x86_64)").size(20.0).strong());
            ui.label("Mounted at /mnt/host when the image starts.");
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
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Images").size(28.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(design::secondary("Build image…")).clicked() {
                    open_popup(ui.ctx(), "Build x86_64 image", &mut self.build_open);
                }
                if ui.add(design::secondary("Open folder…")).clicked() {
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
                if ui.button("Refresh").clicked() {
                    self.status = self.refresh().err().unwrap_or_default();
                }
            });
        });
        ui.add_space(9.0);
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.images.is_empty() {
                ui.label("No images yet");
                ui.label(
                    RichText::new("Open an image folder or build an image to get started.")
                        .color(MUTED),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("image-list")
                    .max_height(210.0)
                    .show(ui, |ui| {
                        for image in &self.images {
                            let name = image.path.file_name().unwrap_or_default().to_string_lossy();
                            let selected = self.selected.as_ref() == Some(&image.path);
                            let row = egui::Button::new(format!("{name}    ·    {}", image.arch))
                                .selected(selected)
                                .fill(if selected {
                                    Color32::from_rgb(228, 237, 252)
                                } else {
                                    Color32::WHITE
                                })
                                .min_size(egui::vec2(ui.available_width(), 40.0));
                            if ui
                                .add(row)
                                .on_hover_text(image.path.display().to_string())
                                .clicked()
                            {
                                self.selected = Some(image.path.clone());
                            }
                        }
                    });
            }
        });
        ui.add_space(14.0);
        let Some(dir) = self.selected.clone() else {
            return Ok(());
        };
        let arm = guest_arch(&dir) == Some("armv7l");
        #[cfg(windows)]
        let lock = dir.join(".ornn-session");
        #[cfg(target_os = "linux")]
        let lock = if arm {
            dir.join("build.lock")
        } else {
            let raw = dir.join("tizen-rootfs.ext4");
            if raw.is_file() {
                raw
            } else {
                dir.join("tizen-rootfs.qcow2")
            }
        };
        let running = match std::fs::File::open(lock) {
            Ok(file) => match file.try_lock_shared() {
                Ok(()) => false,
                Err(std::fs::TryLockError::WouldBlock) => true,
                Err(std::fs::TryLockError::Error(e)) => return Err(e.to_string()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.to_string()),
        };
        let stopping = self.stopping.as_ref().is_some_and(|(path, _)| path == &dir);
        let path = dir.to_str().ok_or("Invalid image directory")?;
        design::card()
            .show(ui, |ui| -> Result<(), String> {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(dir.file_name().unwrap_or_default().to_string_lossy())
                            .size(18.0),
                    );
                    ui.label(if stopping {
                        "Shutting down…"
                    } else if running {
                        "Running"
                    } else {
                        "Stopped"
                    });
                });
                ui.label(RichText::new(path).color(MUTED));
                ui.add_space(6.0);
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
                                args.extend(["--arch", "armv7l"]);
                            }
                            self.terminal(&args)?;
                            self.status.clear();
                        }
                        if !arm
                            && ui
                                .add_enabled(!stopping, design::secondary("Start terminal only"))
                                .clicked()
                        {
                            self.terminal(&["run", "--headless", "--image-dir", path])?;
                            self.status.clear();
                        }
                    }
                    if ui
                        .add_enabled(
                            running && self.stopping.is_none(),
                            design::secondary("Shut down"),
                        )
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
                .inner
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
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(24, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.image((self.logo.id(), egui::vec2(30.0, 30.0)));
                    ui.label(RichText::new("Tizen Ornn").size(19.0));
                    ui.label(RichText::new(tizen_ornn::VERSION).size(13.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(design::secondary("Settings")).clicked() {
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
            let closed = popup(
                ctx,
                "Build x86_64 image",
                [480.0, 210.0],
                [320.0, 180.0],
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.label("Snapshot repository URL");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.repository)
                                .desired_width(f32::INFINITY)
                                .hint_text("Leave blank for the configured snapshot"),
                        );
                        ui.label("Image name");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.name)
                                .desired_width(f32::INFINITY)
                                .hint_text("Use the snapshot ID when a URL is supplied"),
                        );
                        if ui.add(design::action("Build")).clicked() {
                            let repository = app.repository.trim().to_owned();
                            let name = app.name.trim().to_owned();
                            let mut args = vec!["build"];
                            if !repository.is_empty() {
                                args.extend(["--repository", &repository]);
                            }
                            if !name.is_empty() {
                                args.extend(["--name", &name]);
                            }
                            match app.terminal(&args) {
                                Ok(()) => app.build_open = false,
                                Err(e) => app.status = e,
                            }
                        }
                    });
                },
            );
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
            .with_inner_size([760.0, 560.0])
            .with_min_inner_size([540.0, 350.0]),
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
