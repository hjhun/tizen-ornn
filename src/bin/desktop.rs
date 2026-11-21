#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "desktop/design.rs"]
mod design;
use design::{Artwork, INK, MUTED};
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

#[derive(Default)]
struct Launcher {
    images: Vec<Image>,
    selected: Option<PathBuf>,
    custom: String,
    #[cfg(windows)]
    name: String,
    workdir: String,
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
        if !self
            .images
            .iter()
            .any(|i| Some(&i.path) == self.selected.as_ref())
        {
            self.selected = self.images.first().map(|i| i.path.clone());
        }
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
        ui.label(RichText::new("Settings").size(31.0).strong());
        ui.add_space(10.0);
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
                    Ok(()) => {
                        self.custom.clear();
                        self.refresh()
                            .err()
                            .unwrap_or_else(|| "Work directory saved.".into())
                    }
                    Err(e) => format!("Could not save work directory: {e}"),
                };
            }
        });
    }

    fn build_controls(&mut self, ui: &mut egui::Ui) {
        #[cfg(windows)]
        {
            ui.label("New image name");
            ui.text_edit_singleline(&mut self.name);
        }
        if ui.add(design::action("Build image…")).clicked() {
            #[cfg(windows)]
            {
                let name = self.name.clone();
                if let Err(e) = self.terminal(&["build", "--name", &name]) {
                    self.status = e;
                }
            }
            #[cfg(target_os = "linux")]
            if let Err(e) = self.terminal(&["build"]) {
                self.status = e;
            }
        }
    }

    fn library(&mut self, ui: &mut egui::Ui, art: &Artwork) -> Result<(), String> {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Image library").size(31.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(design::secondary("Refresh")).clicked() {
                    self.status = self.refresh().err().unwrap_or_default();
                }
            });
        });
        ui.add_space(5.0);
        if self.images.is_empty() {
            design::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let width = (ui.available_width() * 0.32).min(190.0);
                    ui.add(egui::Image::new((art.library.id(), egui::vec2(width, width * 0.5625))).corner_radius(6));
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("No images yet").size(21.0).strong());
                        ui.label(RichText::new("Build a development image or open an existing image directory below.").color(MUTED));
                        self.build_controls(ui);
                    });
                });
            });
        } else {
            design::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt("image-list")
                    .max_height(164.0)
                    .show(ui, |ui| {
                        for image in &self.images {
                            let name = image.path.file_name().unwrap_or_default().to_string_lossy();
                            let selected = self.custom.is_empty()
                                && self.selected.as_ref() == Some(&image.path);
                            let row = egui::Button::new(format!("{name}   ·   {}", image.arch))
                                .selected(selected)
                                .min_size(egui::vec2(ui.available_width(), 34.0));
                            if ui
                                .add(row)
                                .on_hover_text(image.path.display().to_string())
                                .clicked()
                            {
                                self.selected = Some(image.path.clone());
                                self.custom.clear();
                            }
                        }
                    });
                self.build_controls(ui);
            });
        }
        ui.add_space(12.0);
        ui.label(RichText::new("Tizen account terminal").size(18.0).strong());
        ui.label(RichText::new("Image directory").color(MUTED));
        ui.add(
            egui::TextEdit::singleline(&mut self.custom)
                .margin(egui::Margin::symmetric(10, 7))
                .desired_width(f32::INFINITY)
                .hint_text(
                    self.selected
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Enter a directory containing a guest disk".into()),
                ),
        );
        let directory = if self.custom.is_empty() {
            self.selected.clone()
        } else {
            Some(PathBuf::from(&self.custom))
        };
        if ui
            .add_enabled(
                directory.is_some(),
                design::secondary("Open tizen terminal"),
            )
            .clicked()
        {
            let dir = directory.unwrap();
            let mut args = Vec::new();
            #[cfg(target_os = "linux")]
            if self
                .images
                .iter()
                .any(|i| i.path == dir && i.arch == "armv7l")
            {
                args.extend(["--arch", "armv7l", "run"]);
            }
            if args.is_empty() {
                args.extend(["run", "--headless"]);
            }
            args.extend([
                "--image-dir",
                dir.to_str().ok_or("Invalid image directory")?,
            ]);
            self.terminal(&args)?;
        }
        Ok(())
    }
}

struct Desktop {
    launcher: Launcher,
    art: Artwork,
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
        if !app.processes.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(INK)
                    .inner_margin(egui::Margin::symmetric(22, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.image((self.art.mark.id(), egui::vec2(56.0, 56.0)));
                    ui.label(design::wordmark());
                    ui.label(
                        RichText::new(tizen_ornn::VERSION)
                            .color(Color32::from_rgb(174, 203, 198))
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if app.settings_open {
                            "Back to library"
                        } else {
                            "Settings"
                        };
                        if ui.add(design::secondary(label)).clicked() {
                            app.settings_open = !app.settings_open;
                            app.status.clear();
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
                        if app.settings_open {
                            app.settings(ui);
                        } else if let Err(error) = app.library(ui, &self.art) {
                            app.status = error;
                        }
                        if !app.status.is_empty() {
                            ui.add_space(8.0);
                            ui.label(RichText::new(&app.status).color(INK));
                        }
                    });
            });
    }
}

fn main() -> eframe::Result {
    let mut app = Launcher::default();
    #[cfg(windows)]
    {
        app.name = "default".into();
    }
    if let Ok(Some(path)) = tizen_ornn::workdir::setting() {
        app.workdir = path.display().to_string();
    }
    if let Err(e) = app.refresh() {
        app.status = e;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("tizen-ornn")
            .with_icon(eframe::icon_data::from_png_bytes(design::ICON).expect("embedded app icon"))
            .with_inner_size([680.0, 485.0])
            .with_min_inner_size([400.0, 330.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tizen Ornn",
        options,
        Box::new(move |cc| {
            design::configure(&cc.egui_ctx);
            Ok(Box::new(Desktop {
                launcher: app,
                art: Artwork::load(&cc.egui_ctx),
            }))
        }),
    )
}
