#![cfg_attr(windows, windows_subsystem = "windows")]

use eframe::egui;
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
    status: String,
    processes: Vec<Child>,
}

impl Launcher {
    fn refresh(&mut self) -> Result<(), String> {
        let output = cli()?.arg("images").output().map_err(|e| e.to_string())?;
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
            c.arg("-e").arg(path);
            c
        };
        let child = cmd
            .args(args)
            .stdin(Stdio::inherit())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.processes.push(child);
        Ok(())
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        ui.horizontal(|ui| {
            ui.heading("Development images");
            if ui.button("Refresh").clicked() {
                self.status = self.refresh().err().unwrap_or_default();
            }
        });
        egui::ScrollArea::vertical()
            .max_height(195.0)
            .show(ui, |ui| {
                for image in &self.images {
                    let label = format!(
                        "{}   ·   {}",
                        image.path.file_name().unwrap_or_default().to_string_lossy(),
                        image.arch
                    );
                    if ui
                        .selectable_label(self.selected.as_ref() == Some(&image.path), label)
                        .on_hover_text(image.path.display().to_string())
                        .clicked()
                    {
                        self.selected = Some(image.path.clone());
                        self.custom.clear();
                    }
                }
                if self.images.is_empty() {
                    ui.label("Create an image or enter an existing image directory.");
                }
            });
        ui.separator();
        ui.label("Image directory");
        ui.add(
            egui::TextEdit::singleline(&mut self.custom)
                .desired_width(f32::INFINITY)
                .hint_text(
                    self.selected
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
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
                egui::Button::new("Open guest terminal"),
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
        ui.separator();
        #[cfg(windows)]
        {
            ui.label("New image name");
            ui.text_edit_singleline(&mut self.name);
        }
        if ui.button("Build image…").clicked() {
            #[cfg(windows)]
            {
                let name = self.name.clone();
                self.terminal(&["build", "--name", &name])?;
            }
            #[cfg(target_os = "linux")]
            self.terminal(&["build"])?;
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        Ok(())
    }
}

impl eframe::App for Launcher {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let mut finished = false;
        self.processes.retain_mut(|p| match p.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                if !status.success() {
                    self.status = format!("Terminal exited with {status}");
                }
                finished = true;
                false
            }
            Err(e) => {
                self.status = e.to_string();
                false
            }
        });
        if finished {
            if let Err(e) = self.refresh() {
                self.status = e;
            }
        }
        if !self.processes.is_empty() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Err(e) = self.ui(ui) {
                self.status = e;
            }
        });
    }
}

fn main() -> eframe::Result {
    let mut app = Launcher::default();
    #[cfg(windows)]
    {
        app.name = "default".into();
    }
    if let Err(e) = app.refresh() {
        app.status = e;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([540.0, 410.0]),
        ..Default::default()
    };
    eframe::run_native("Tizen Ornn", options, Box::new(move |_| Ok(Box::new(app))))
}
