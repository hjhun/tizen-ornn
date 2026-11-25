#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "desktop/design.rs"]
mod design;
#[path = "desktop/tray.rs"]
mod tray;
use design::{INK, MUTED};
use tizen_ornn::workdir::Language;
#[path = "desktop/korean.rs"]
mod korean;
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

#[derive(Clone, Copy)]
enum Picker {
    ImageFolder,
    WorkFolder,
    SharedFolder,
    Certificate,
}

impl Picker {
    fn title(self, lang: Language) -> &'static str {
        match self {
            Self::ImageFolder => lang.text("Open image folder", "이미지 폴더 열기"),
            Self::Certificate => lang.text("Choose CA certificate", "CA 인증서 선택"),
            Self::WorkFolder => lang.text("Choose work directory", "작업 폴더 선택"),
            Self::SharedFolder => lang.text("Choose shared folder", "공유 폴더 선택"),
        }
    }
}

#[derive(Default)]
struct Launcher {
    language: Language,
    images: Vec<Image>,
    external: Vec<PathBuf>,
    removal: Option<Removal>,
    selected: Option<PathBuf>,
    stopping: Option<(PathBuf, Child)>,
    picker: Option<(Picker, egui_file_dialog::FileDialog)>,
    ca_path: String,
    manual_proxy: bool,
    proxy: tizen_ornn::proxy::Manual,
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
    update_process: Option<Child>,
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
                Err(e) => {
                    self.status = format!(
                        "{}: {e}",
                        self.language.text(
                            "Could not start notification icon",
                            "알림 아이콘을 표시할 수 없습니다"
                        )
                    )
                }
            }
        }
    }

    fn update_trays(&mut self) {
        let lang = self.language;
        if self
            .last_tray_check
            .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.last_tray_check = Some(std::time::Instant::now());
        self.monitors.retain_mut(|child| match child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                self.status = lang
                    .text(
                        "Desktop notification service unavailable.",
                        "데스크톱 알림 서비스를 사용할 수 없습니다.",
                    )
                    .into();
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
        let lang = self.language;
        let arch = guest_arch(&path).ok_or_else(|| {
            std::io::Error::other(lang.text(
                "Choose an image directory containing a guest kernel and disk.",
                "게스트 커널과 디스크가 있는 이미지 폴더를 선택하세요.",
            ))
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
        let child = self.spawn_terminal(args)?;
        self.processes.push(child);
        Ok(())
    }

    fn spawn_terminal(&self, args: &[&str]) -> Result<Child, String> {
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
        cmd.args(args)
            .stdin(Stdio::inherit())
            .spawn()
            .map_err(|e| e.to_string())
    }

    fn build_form(&mut self, ui: &mut egui::Ui) {
        let lang = self.language;
        ui.label(lang.text("Guest architecture", "게스트 아키텍처"));
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
                "{} {arch} + generic HAL · tizen-10.1-unified_20261119.053314",
                lang.text(
                    "Official Tizen 10.1 headed",
                    "공식 Tizen 10.1 그래픽 이미지"
                )
            ));
            ui.label(lang.text("Graphical boot with a serial terminal. The kernel version comes from the matching ARM boot image’s .packages file. This build uses the configured snapshot and the image name shown above. Custom RPM repositories, local archives, host folders and package updates are unsupported.", "시리얼 터미널과 그래픽 화면으로 부팅합니다. 커널 버전은 해당 ARM 부팅 이미지의 .packages 파일에서 가져옵니다. 위의 스냅샷과 이미지 이름을 사용하며 사용자 지정 RPM 저장소, 로컬 아카이브, 호스트 폴더 및 패키지 업데이트는 지원하지 않습니다."));
            if cfg!(windows) {
                ui.label(
                    lang.text("Windows uses a disposable Ubuntu builder VM and native QEMU for the guest.", "Windows에서는 임시 Ubuntu VM으로 빌드하고 Windows용 QEMU로 게스트를 실행합니다."),
                );
            } else {
                ui.label(lang.text(
                    "Install qemu-system-arm and the matching cross compiler before building.",
                    "빌드하기 전에 qemu-system-arm과 대상에 맞는 크로스 컴파일러를 설치하세요.",
                ));
            }
        } else {
            ui.label(lang.text("The builder downloads the official source archive and development RPMs. Use an HTTPS snapshot or compatible mirror. Additional RPM repository URLs are unsupported. For a local archive, use scripts/tizen-image --archive on Ubuntu.", "공식 이미지 아카이브와 개발용 RPM을 다운로드합니다. HTTPS 스냅샷 또는 호환 미러 주소를 입력하세요. 추가 RPM 저장소는 지원하지 않습니다. 로컬 아카이브는 Ubuntu에서 scripts/tizen-image --archive로 빌드할 수 있습니다."));
            ui.label(lang.text("Snapshot repository URL", "스냅샷 저장소 URL"));
            ui.add(
                egui::TextEdit::singleline(&mut self.repository)
                    .desired_width(f32::INFINITY)
                    .hint_text("https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/"),
            );
            ui.label(lang.text("Image name", "이미지 이름"));
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .desired_width(f32::INFINITY)
                    .hint_text(lang.text(
                        "Leave blank to use the snapshot ID, including dots",
                        "비워 두면 점을 포함한 스냅샷 ID를 사용합니다",
                    )),
            );
        }
        if ui.add(design::action(lang.text("Build", "빌드"))).clicked() {
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

    fn open_picker(&mut self, ctx: &egui::Context, purpose: Picker) {
        if let Some((open, _)) = &self.picker {
            ctx.send_viewport_cmd_to(
                popup_id(open.title(Language::En)),
                egui::ViewportCommand::Focus,
            );
            return;
        }
        let mut dialog = egui_file_dialog::FileDialog::new()
            .title_bar(false)
            .fixed_pos(egui::Pos2::ZERO)
            .resizable(false);
        if self.language == Language::Ko {
            dialog = dialog.labels(korean::dialog_labels());
        }
        let start = match purpose {
            Picker::WorkFolder => {
                if std::path::Path::new(&self.workdir).is_absolute() {
                    PathBuf::from(&self.workdir)
                } else {
                    PathBuf::from(&self.active_workdir)
                }
            }
            Picker::SharedFolder if self.share.is_empty() => {
                tizen_ornn::workdir::default_share().unwrap_or_default()
            }
            Picker::SharedFolder => PathBuf::from(&self.share),
            Picker::Certificate => PathBuf::from(&self.ca_path),
            Picker::ImageFolder => PathBuf::from(&self.active_workdir),
        };
        if let Some(dir) = start.ancestors().find(|path| path.is_dir()) {
            dialog = dialog.initial_directory(dir.to_owned());
        }
        match purpose {
            Picker::ImageFolder | Picker::WorkFolder | Picker::SharedFolder => {
                dialog.pick_directory()
            }
            Picker::Certificate => dialog.pick_file(),
        }
        self.picker = Some((purpose, dialog));
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        let lang = self.language;
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut choice = self.language;
            ui.horizontal_wrapped(|ui| {
                ui.label(lang.text("Language", "언어"));
                ui.selectable_value(&mut choice, Language::En, "English");
                ui.selectable_value(&mut choice, Language::Ko, "한국어");
            });
            if choice != self.language {
                match tizen_ornn::workdir::save_language(choice) {
                    Ok(()) => { self.language = choice; self.status.clear(); }
                    Err(e) => self.status = e.to_string(),
                }
            }
            ui.separator();
            ui.label(design::heading(lang.text("Work directory", "작업 폴더"), 20.0));
            ui.label(
                RichText::new(lang.text("Images, build files and downloads are stored here.", "이미지, 빌드 파일 및 다운로드를 저장할 폴더입니다.")).color(MUTED),
            );
            let default = tizen_ornn::workdir::default_dir().unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut self.workdir)
                    .margin(egui::Margin::symmetric(10, 7))
                    .hint_text(default.display().to_string())
                    .desired_width(f32::INFINITY),
            );
            ui.label(
                RichText::new(format!("{}: {}", lang.text("Current folder", "현재 폴더"), self.active_workdir)).color(MUTED),
            );
            ui.horizontal_wrapped(|ui| {
                if ui.add(design::secondary(lang.text("Browse…", "찾아보기…"))).clicked() {
                    self.open_picker(ui.ctx(), Picker::WorkFolder);
                }
            if ui.add(design::action(lang.text("Save work directory", "작업 폴더 저장"))).clicked() {
                self.status = match tizen_ornn::workdir::save(&self.workdir) {
                    Ok(()) => self
                        .refresh()
                        .err()
                        .unwrap_or_else(|| lang.text("Work directory saved.", "작업 폴더를 저장했습니다.").into()),
                    Err(e) => format!("{}: {e}", lang.text("Could not save work directory", "작업 폴더를 저장할 수 없습니다")),
                };
            }
            });
            ui.separator();
            ui.label(design::heading(lang.text("Build CA certificate", "빌드용 CA 인증서"), 20.0));
            ui.label(lang.text("Choose a public PEM certificate for new images. Leave blank to use TIZEN_BUILD_CA_CERT.", "새 이미지 빌드에 사용할 공개 PEM 인증서를 선택하세요. 비워 두면 TIZEN_BUILD_CA_CERT를 사용합니다."));
            ui.add(egui::TextEdit::singleline(&mut self.ca_path)
                .desired_width(f32::INFINITY).hint_text(lang.text("Path to company-root.crt", "company-root.crt 파일 경로")));
            ui.horizontal(|ui| {
                if ui.add(design::secondary(lang.text("Browse…", "찾아보기…"))).clicked() {
                    self.open_picker(ui.ctx(), Picker::Certificate);
                }
                if ui.add(design::action(lang.text("Save certificate", "인증서 저장"))).clicked() {
                    self.status = match tizen_ornn::workdir::save_ca(&self.ca_path) {
                        Ok(()) if self.ca_path.is_empty() => lang.text("Certificate setting cleared.", "인증서 설정을 지웠습니다.").into(),
                        Ok(()) => lang.text("Certificate saved for new image builds.", "새 이미지 빌드에 사용할 인증서를 저장했습니다.").into(),
                        Err(error) => error.to_string(),
                    };
                }
            });
            ui.separator();
            ui.label(design::heading(lang.text("Build proxy", "빌드 프록시"), 20.0));
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.manual_proxy, false, lang.text("System settings", "시스템 설정"));
                ui.radio_value(&mut self.manual_proxy, true, lang.text("Manual proxy", "수동 프록시"));
            });
            if self.manual_proxy {
                ui.label(lang.text("HTTP / HTTPS proxy address", "HTTP / HTTPS 프록시 주소"));
                ui.add(egui::TextEdit::singleline(&mut self.proxy.url)
                    .desired_width(f32::INFINITY).hint_text("http://proxy.example.org:8080"));
                ui.label(lang.text("Bypass proxy for", "프록시 제외 대상"));
                ui.add(egui::TextEdit::singleline(&mut self.proxy.bypass)
                    .desired_width(f32::INFINITY).hint_text("localhost,.example.org"));
            }
            if ui.add(design::action(lang.text("Save proxy", "프록시 저장"))).clicked() {
                self.status = match tizen_ornn::workdir::save_proxy(self.manual_proxy.then_some(&self.proxy)) {
                    Ok(()) if self.manual_proxy => lang.text("Manual proxy saved for downloads and builds.", "다운로드와 빌드에 사용할 수동 프록시를 저장했습니다.").into(),
                    Ok(()) => lang.text("Downloads and builds will use the system proxy settings.", "다운로드와 빌드에 시스템 프록시 설정을 사용합니다.").into(),
                    Err(e) => e.to_string(),
                };
            }
            ui.separator();
            ui.label(design::heading(lang.text("Shared folder (x86_64)", "공유 폴더 (x86_64)"), 20.0));
            ui.label(lang.text("Available at /home/tizen/share and /mnt/host when the image starts.", "부팅하면 /home/tizen/share 및 /mnt/host에서 이 폴더를 사용할 수 있습니다."));
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
            ui.horizontal_wrapped(|ui| {
                if ui.add(design::secondary(lang.text("Browse…", "찾아보기…"))).clicked() {
                    self.open_picker(ui.ctx(), Picker::SharedFolder);
                }
            if ui.add(design::action(lang.text("Save shared folder", "공유 폴더 저장"))).clicked() {
                self.status = match tizen_ornn::workdir::save_share(&self.share) {
                    Ok(()) => lang.text("Shared folder saved for the next boot.", "다음 부팅에 사용할 공유 폴더를 저장했습니다.").into(),
                    Err(error) => error.to_string(),
                };
            }
            });
        });
    }

    fn library(&mut self, ui: &mut egui::Ui) -> Result<(), String> {
        let lang = self.language;
        let wide = ui.available_width() >= 740.0;
        if wide {
            ui.horizontal(|ui| {
                ui.label(design::heading(
                    lang.text("Image library", "이미지 목록"),
                    30.0,
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.library_actions(ui)
                });
            });
        } else {
            ui.label(design::heading(
                lang.text("Image library", "이미지 목록"),
                26.0,
            ));
            ui.horizontal_wrapped(|ui| self.library_actions(ui));
        }
        ui.add_space(8.0);
        if self.images.is_empty() {
            design::card().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(design::heading(
                    lang.text("No images yet", "이미지가 없습니다"),
                    20.0,
                ));
                ui.label(lang.text(
                    "Open a folder containing a guest image or build one from a snapshot.",
                    "게스트 이미지가 있는 폴더를 열거나 스냅샷에서 이미지를 빌드하세요.",
                ));
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
        let lang = self.language;
        let open_button = if self.images.is_empty() {
            design::action(lang.text("Add image…", "이미지 추가…"))
        } else {
            design::secondary(lang.text("Add image…", "이미지 추가…"))
        };
        let open = ui.add(open_button).clicked();
        let build = ui
            .add(design::secondary(lang.text("Build image…", "이미지 빌드…")))
            .clicked();
        if ui
            .add(egui::Button::new(lang.text("Refresh", "새로 고침")).frame(false))
            .clicked()
        {
            self.status = self.refresh().err().unwrap_or_default();
        }
        if build {
            open_popup(ui.ctx(), "Build image", &mut self.build_open);
        }
        if open {
            self.open_picker(ui.ctx(), Picker::ImageFolder);
        }
    }

    fn image_list(&mut self, ui: &mut egui::Ui) {
        design::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(design::heading(
                format!(
                    "{} ({})",
                    self.language.text("Images", "이미지"),
                    self.images.len()
                ),
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
        let lang = self.language;
        let Some(dir) = self.selected.clone() else {
            return Ok(());
        };
        let arch = guest_arch(&dir)
            .ok_or(lang.text("Image files are missing.", "이미지 파일이 없습니다."))?;
        let arm = arch != "x86_64";
        let running = tray::running(&dir).map_err(|e| e.to_string())?;
        let stopping = self.stopping.as_ref().is_some_and(|(path, _)| path == &dir);
        let path = dir.to_str().ok_or(lang.text(
            "Invalid image directory",
            "이미지 폴더가 올바르지 않습니다.",
        ))?;
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
                        (lang.text("Shutting down…", "종료 중…"), Color32::from_rgb(169, 111, 25))
                    } else if running {
                        (lang.text("Running", "실행 중"), Color32::from_rgb(32, 129, 92))
                    } else {
                        (lang.text("Stopped", "정지됨"), MUTED)
                    };
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                    ui.label(RichText::new(text).size(14.0).color(MUTED));
                });
                ui.horizontal_wrapped(|ui| -> Result<(), String> {
                    if running {
                        if ui
                            .add_enabled(!stopping, design::action(lang.text("Open terminal", "터미널 열기")))
                            .clicked()
                        {
                            self.terminal(&["console", "--image-dir", path])?;
                            self.status.clear();
                        }
                    } else {
                        if ui
                            .add_enabled(!stopping, design::action(lang.text("Start with display", "그래픽 화면으로 시작")))
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
                                .add_enabled(!stopping, design::secondary(lang.text("Start terminal only", "터미널로 시작")))
                                .clicked()
                        {
                            self.terminal(&["run", "--headless", "--image-dir", path])?;
                            self.attach_tray(&dir);
                            self.status.clear();
                        }
                    }
                    if !arm && !running && ui
                        .add_enabled(!stopping && self.update_process.is_none(), design::secondary(lang.text("Update image", "이미지 업데이트")))
                        .on_hover_text(lang.text("Build the latest x86_64 snapshot and keep the previous disk.", "최신 x86_64 스냅샷을 빌드하고 이전 디스크도 보관합니다."))
                        .clicked()
                    {
                        self.update_process = Some(self.spawn_terminal(&["update", "--image", "--image-dir", path])?);
                        self.status = lang.text("Updating the selected image. The library refreshes when the terminal finishes.", "선택한 이미지를 업데이트하고 있습니다. 터미널이 종료되면 목록을 새로 고칩니다.").into();
                    }
                    if running
                        && ui
                            .add_enabled(self.stopping.is_none(), design::secondary(lang.text("Shut down", "종료")))
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
                ui.label(RichText::new(lang.text("Image folder", "이미지 폴더")).color(MUTED).size(13.0));
                ui.add(egui::Label::new(path).wrap());
                ui.label(RichText::new(lang.text("Architecture", "아키텍처")).color(MUTED).size(13.0));
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
                                lang.text("Disconnect image…", "이미지 연결 해제…")
                            } else {
                                lang.text("Delete image…", "이미지 삭제…")
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
    id: &str,
    title: &str,
    size: [f32; 2],
    minimum: [f32; 2],
    mut draw: impl FnMut(&egui::Context),
) -> bool {
    ctx.show_viewport_immediate(
        popup_id(id),
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
        let lang = app.language;
        app.update_trays();
        let mut finished = false;
        app.processes.retain_mut(|p| match p.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                if !status.success() {
                    app.status =
                        format!("{}: {status}", lang.text("Terminal exited", "터미널 종료"));
                }
                finished = true;
                false
            }
            Err(e) => {
                app.status = e.to_string();
                false
            }
        });
        if let Some(result) = app
            .update_process
            .as_mut()
            .and_then(|p| p.try_wait().transpose())
        {
            app.update_process = None;
            finished = true;
            app.status = match result {
                Ok(status) if status.success() => lang
                    .text(
                        "Update finished. Select the updated image in the library.",
                        "업데이트가 끝났습니다. 목록에서 새 이미지를 선택하세요.",
                    )
                    .into(),
                Ok(status) => format!(
                    "{}: {status}",
                    lang.text("Image update exited", "이미지 업데이트 종료")
                ),
                Err(e) => e.to_string(),
            };
        }
        if finished && let Err(e) = app.refresh() {
            app.status = e;
        }
        if let Some((_, proc)) = app.stopping.as_mut() {
            match proc.try_wait() {
                Ok(Some(_)) => {
                    let (_, child) = app.stopping.take().unwrap();
                    app.status = match child.wait_with_output() {
                        Ok(out) if out.status.success() => lang
                            .text("Guest shut down.", "게스트가 종료되었습니다.")
                            .into(),
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
        if app.selected.is_some() || !app.processes.is_empty() || app.update_process.is_some() {
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
                                egui::Button::new(
                                    RichText::new(lang.text("Settings", "설정"))
                                        .color(Color32::WHITE),
                                )
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
                lang.text("Disconnect image", "이미지 연결 해제")
            } else {
                lang.text("Delete image", "이미지 삭제")
            };
            let mut confirm = false;
            let mut cancel = false;
            let closed = popup(
                ctx,
                "remove-image",
                title,
                [460.0, 205.0],
                [320.0, 170.0],
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.label(design::heading(title, 22.0));
                            ui.label(if removal.external {
                                lang.text(
                                    "Remove this folder from the library and keep its files.",
                                    "파일을 보관하고 이 폴더를 이미지 목록에서 제거합니다.",
                                )
                            } else {
                                lang.text(
                                "Permanently delete the guest disk and all data stored inside it?",
                                "게스트 디스크와 그 안의 모든 데이터를 영구 삭제할까요?",
                            )
                            });
                            ui.add(egui::Label::new(removal.image.display().to_string()).wrap());
                            ui.horizontal(|ui| {
                                cancel = ui
                                    .add(design::secondary(lang.text("Cancel", "취소")))
                                    .clicked();
                                confirm = ui
                                    .add(design::secondary(if removal.external {
                                        lang.text("Disconnect", "연결 해제")
                                    } else {
                                        lang.text("Delete disk", "디스크 삭제")
                                    }))
                                    .clicked();
                            });
                        });
                    });
                },
            );
            if closed || cancel || confirm {
                app.removal = None;
            }
            if confirm && !closed {
                app.status = (|| -> Result<String, String> {
                    if removal.external {
                        update_connection(&removal.store, &removal.image, false)
                            .map_err(|e| e.to_string())?;
                    } else {
                        let name = removal
                            .image
                            .file_name()
                            .ok_or(lang.text("Missing image name", "이미지 이름이 없습니다"))?;
                        let mut cmd = cli()?;
                        cmd.env("TIZEN_ORNN_HOME", &removal.store)
                            .env_remove("TIZEN_OUT");
                        #[cfg(target_os = "linux")]
                        cmd.args([
                            "--arch",
                            guest_arch(&removal.image).ok_or(lang.text(
                                "Image has changed. Refresh the library.",
                                "이미지가 변경되었습니다. 목록을 새로 고치세요.",
                            ))?,
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
                        lang.text("Image disconnected.", "이미지 연결을 해제했습니다.")
                    } else {
                        lang.text("Guest disk deleted.", "게스트 디스크를 삭제했습니다.")
                    }
                    .into())
                })()
                .unwrap_or_else(|error| error);
            }
        }
        if app.settings_open {
            let closed = popup(
                ctx,
                "Settings",
                lang.text("Settings", "설정"),
                [560.0, 540.0],
                [380.0, 250.0],
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            app.settings(ui);
                            if !app.status.is_empty() {
                                ui.label(&app.status);
                            }
                        });
                    });
                },
            );
            app.settings_open = !closed;
        }
        if app.build_open {
            let closed = popup(
                ctx,
                "Build image",
                lang.text("Build image", "이미지 빌드"),
                [550.0, 390.0],
                [320.0, 180.0],
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            app.build_form(ui);
                        });
                    });
                },
            );
            app.build_open &= !closed;
        }
        if let Some((purpose, dialog)) = &mut app.picker {
            let closed = popup(
                ctx,
                purpose.title(Language::En),
                purpose.title(lang),
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
                let purpose = *purpose;
                app.picker = None;
                match purpose {
                    Picker::ImageFolder => {
                        app.status = app
                            .connect_directory(path)
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_default();
                    }
                    Picker::WorkFolder => app.workdir = path.display().to_string(),
                    Picker::SharedFolder => app.share = path.display().to_string(),
                    Picker::Certificate => app.ca_path = path.display().to_string(),
                }
            } else if closed || dialog.state() != egui_file_dialog::DialogState::Open {
                app.picker = None;
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
    let mut app = Launcher {
        language: tizen_ornn::workdir::language().unwrap_or_default(),
        ..Default::default()
    };
    if let Ok(Some(path)) = tizen_ornn::workdir::setting() {
        app.workdir = path.display().to_string();
    }
    if let Ok(Some(proxy)) = tizen_ornn::workdir::proxy_setting() {
        app.manual_proxy = true;
        app.proxy = proxy;
    }
    if let Ok(Some(path)) = tizen_ornn::workdir::ca_setting() {
        app.ca_path = path.display().to_string();
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
