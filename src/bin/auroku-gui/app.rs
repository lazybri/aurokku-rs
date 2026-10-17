//! Material 3 egui front-end for auRokku.

use std::time::{Duration, Instant};

use auroku::{BatchProfile, DeviceConfig, DeviceSummary, ProbeKind, Task};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, Context, FontId, Frame, Id, Layout, Margin, Rect,
    RichText, ScrollArea, Sense, Stroke, Ui,
};
use serde_json::Value;

use crate::i18n::{self, Language};
use crate::theme::{self, M3Colors};
use crate::widgets::{
    self, banner, button, card, card_header, chip, code_block, empty_state, icon_button, kv_row,
    metric, page_header, segmented, subtle_label, switch, text_field, ButtonKind, Icon, Tone,
};
use crate::worker::{ApiError, Event, Request, Worker};

// ==============================================================================
// Screens
// ==============================================================================

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Screen {
    Overview,
    Uim,
    Pin,
    Probe,
    Batch,
    Session,
    Console,
}

impl Screen {
    const ALL: [Screen; 7] = [
        Screen::Overview,
        Screen::Uim,
        Screen::Pin,
        Screen::Probe,
        Screen::Batch,
        Screen::Session,
        Screen::Console,
    ];

    fn label(self) -> &'static str {
        match self {
            Screen::Overview => i18n::t("概览"),
            Screen::Uim => i18n::t("UIM 锁"),
            Screen::Pin => "PIN / ICCID",
            Screen::Probe => i18n::t("探测"),
            Screen::Batch => i18n::t("批量"),
            Screen::Session => i18n::t("会话"),
            Screen::Console => i18n::t("控制台"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            Screen::Overview => Icon::Dashboard,
            Screen::Uim => Icon::Lock,
            Screen::Pin => Icon::Key,
            Screen::Probe => Icon::Probe,
            Screen::Batch => Icon::List,
            Screen::Session => Icon::Power,
            Screen::Console => Icon::Terminal,
        }
    }
}

// ==============================================================================
// Small state types
// ==============================================================================

#[derive(Clone)]
struct LogLine {
    at: f64,
    text: String,
}

#[derive(Clone)]
struct Snack {
    text: String,
    tone: Tone,
    at: Instant,
}

#[derive(Clone)]
enum Confirm {
    ClearUim,
    ResetPin,
    WriteUim { value: String, control: u8 },
    MaintenanceBit { control: u8 },
}

impl Confirm {
    fn title(&self) -> &'static str {
        match self {
            Confirm::ClearUim => i18n::t("清除 UIM 锁绑定？"),
            Confirm::ResetPin => i18n::t("重置手机锁码？"),
            Confirm::WriteUim { .. } => i18n::t("写入 UIM 锁绑定？"),
            Confirm::MaintenanceBit { .. } => i18n::t("设置维护模式位？"),
        }
    }

    fn body(&self) -> String {
        match self {
            Confirm::ClearUim => i18n::t(
                "此操作会把锁值写为 ffffffff 并解除 SIM 绑定。请确认设备与卡片状态后再继续。",
            )
            .to_owned(),
            Confirm::ResetPin => {
                i18n::t("手机锁码（PIN / NV 82）将恢复为默认值 1234。").to_owned()
            }
            Confirm::WriteUim { value, control } => i18n::tf(
                "将锁值写为 {value}，控制位 {control}。写入后锁会绑定到该值，请确认卡片匹配。",
                &[("value", value), ("control", &control.to_string())],
            ),
            Confirm::MaintenanceBit { control } => i18n::tf(
                "通过 0xE1 服务 + auth 4000 写入 NV 10481，control = {control}。此操作会切换维护模式位。",
                &[("control", &control.to_string())],
            ),
        }
    }

    fn action_label(&self) -> &'static str {
        match self {
            Confirm::ClearUim => i18n::t("清除绑定"),
            Confirm::ResetPin => i18n::t("重置 PIN"),
            Confirm::WriteUim { .. } => i18n::t("写入"),
            Confirm::MaintenanceBit { .. } => i18n::t("应用"),
        }
    }
}

// ==============================================================================
// App
// ==============================================================================

pub struct AurokuApp {
    worker: Worker,

    dark: bool,
    scheme: M3Colors,
    applied_dark: Option<bool>,
    lang: Language,
    cjk_loaded: bool,

    screen: Screen,

    // Device form
    vid_text: String,
    pid_text: String,
    sn_text: String,
    interface_text: String,
    ep_out_text: String,
    ep_in_text: String,
    verbose: bool,
    type2: bool,

    // Discovery
    devices: Vec<DeviceSummary>,
    selected_device: Option<usize>,
    scanning: bool,
    scan_error: Option<String>,

    // Operation state
    busy: bool,
    busy_label: String,
    result: Option<Value>,
    pretty_json: String,
    last_op: Option<String>,
    last_success: Option<bool>,
    last_error: Option<ApiError>,

    // Operation parameters
    writelk_value: String,
    writelk_control: bool,
    maint_control: bool,
    probe_kind: ProbeKind,
    probe_timeout: String,
    batch_profile: BatchProfile,

    // Console
    logs: Vec<LogLine>,
    auto_scroll_logs: bool,
    started: Instant,

    // Overlays
    snackbar: Option<Snack>,
    confirm: Option<Confirm>,
    show_about: bool,
}

impl AurokuApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cjk_loaded = crate::fonts::install(&cc.egui_ctx);

        let scheme = M3Colors::light();
        theme::apply(&cc.egui_ctx, &scheme);

        // A Chinese locale without a usable CJK font would render as boxes;
        // fall back to English in that case.
        let mut lang = i18n::current();
        if !cjk_loaded && lang == Language::ZhCn {
            lang = Language::En;
            i18n::set(lang);
        }

        let worker = Worker::new(cc.egui_ctx.clone());
        worker.send(Request::Scan {
            vid: None,
            pid: None,
        });

        let mut app = Self {
            worker,
            dark: false,
            scheme,
            applied_dark: Some(false),
            lang,
            cjk_loaded,
            screen: Screen::Overview,
            vid_text: "0482".to_owned(),
            pid_text: "0a5c".to_owned(),
            sn_text: String::new(),
            interface_text: String::new(),
            ep_out_text: "04".to_owned(),
            ep_in_text: "84".to_owned(),
            verbose: false,
            type2: false,
            devices: Vec::new(),
            selected_device: None,
            scanning: true,
            scan_error: None,
            busy: false,
            busy_label: String::new(),
            result: None,
            pretty_json: String::new(),
            last_op: None,
            last_success: None,
            last_error: None,
            writelk_value: "80e48ed2".to_owned(),
            writelk_control: true,
            maint_control: true,
            probe_kind: ProbeKind::List,
            probe_timeout: "1.0".to_owned(),
            batch_profile: BatchProfile::Basic,
            logs: Vec::new(),
            auto_scroll_logs: true,
            started: Instant::now(),
            snackbar: None,
            confirm: None,
            show_about: false,
        };
        app.log(i18n::t("界面已启动，正在扫描 USB 总线").to_owned());
        app
    }

    // --------------------------------------------------------------------------
    // Worker events
    // --------------------------------------------------------------------------

    fn poll_events(&mut self) {
        while let Some(event) = self.worker.try_recv() {
            match event {
                Event::ScanStarted => {
                    self.scanning = true;
                    self.scan_error = None;
                    self.log(i18n::t("开始扫描 USB 设备").to_owned());
                }
                Event::Scanned(devices) => {
                    self.scanning = false;
                    self.log(i18n::tf(
                        "扫描完成，发现 {n} 个 USB 设备",
                        &[("n", &devices.len().to_string())],
                    ));
                    self.devices = devices;
                    let vid = parse_hex_u16(&self.vid_text).ok();
                    let pid = parse_hex_u16(&self.pid_text).ok();
                    let matched = self
                        .devices
                        .iter()
                        .position(|device| Some(device.vid) == vid && Some(device.pid) == pid);
                    if let Some(index) = matched {
                        self.apply_device(index, false);
                    } else {
                        self.selected_device = None;
                    }
                }
                Event::ScanFailed(message) => {
                    self.scanning = false;
                    self.scan_error = Some(message.clone());
                    self.log(i18n::tf("扫描失败：{message}", &[("message", &message)]));
                    self.notify(
                        i18n::tf("扫描失败：{message}", &[("message", &message)]),
                        Tone::Error,
                    );
                }
                Event::Started { label } => {
                    self.busy = true;
                    self.busy_label = label;
                }
                Event::Log(line) => self.log(line),
                Event::Finished { label, result } => {
                    self.busy = false;
                    self.busy_label.clear();
                    match result {
                        Ok(value) => {
                            let success = value
                                .get("success")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            self.last_success = Some(success);
                            self.last_op = Some(
                                value
                                    .get("operation")
                                    .and_then(Value::as_str)
                                    .unwrap_or(&label)
                                    .to_owned(),
                            );
                            self.pretty_json =
                                serde_json::to_string_pretty(&value).unwrap_or_default();
                            self.result = Some(value);
                            self.last_error = None;
                            self.log(i18n::tf(
                                "{label} 完成：{status}",
                                &[
                                    ("label", &label),
                                    (
                                        "status",
                                        i18n::t(if success { "成功" } else { "结果为失败" }),
                                    ),
                                ],
                            ));
                            if success {
                                self.notify(
                                    i18n::tf("{label}：成功", &[("label", &label)]),
                                    Tone::Success,
                                );
                            } else {
                                self.notify(
                                    i18n::tf("{label}：结果为失败", &[("label", &label)]),
                                    Tone::Warning,
                                );
                            }
                        }
                        Err(error) => {
                            self.last_success = Some(false);
                            self.last_op = Some(label.clone());
                            self.result = None;
                            self.pretty_json.clear();
                            self.log(i18n::tf(
                                "{label} 错误（{kind}）：{message}",
                                &[
                                    ("label", &label),
                                    ("kind", &error.kind),
                                    ("message", &error.message),
                                ],
                            ));
                            self.notify(
                                i18n::tf(
                                    "{kind}：{message}",
                                    &[("kind", &error.kind), ("message", &error.message)],
                                ),
                                Tone::Error,
                            );
                            self.last_error = Some(error);
                        }
                    }
                }
            }
        }
    }

    // --------------------------------------------------------------------------
    // Helpers
    // --------------------------------------------------------------------------

    fn log(&mut self, text: String) {
        let at = self.started.elapsed().as_secs_f64();
        self.logs.push(LogLine { at, text });
        if self.logs.len() > 800 {
            let drain = self.logs.len() - 800;
            self.logs.drain(0..drain);
        }
    }

    fn notify(&mut self, text: impl Into<String>, tone: Tone) {
        self.snackbar = Some(Snack {
            text: text.into(),
            tone,
            at: Instant::now(),
        });
    }

    fn apply_device(&mut self, index: usize, announce: bool) {
        let Some(device) = self.devices.get(index).cloned() else {
            return;
        };
        self.selected_device = Some(index);
        self.vid_text = format!("{:04x}", device.vid);
        self.pid_text = format!("{:04x}", device.pid);
        if let Some(serial) = &device.serial {
            self.sn_text = serial.clone();
        }
        if announce {
            self.notify(
                i18n::tf(
                    "已选择 {name}（{id}）",
                    &[
                        ("name", &device.display_name()),
                        ("id", &device.id_string()),
                    ],
                ),
                Tone::Primary,
            );
        }
    }

    fn build_config(&self) -> Result<DeviceConfig, String> {
        let vid = parse_hex_u16(&self.vid_text)?;
        let pid = parse_hex_u16(&self.pid_text)?;
        let ep_out = parse_hex_u8(&self.ep_out_text)?;
        let ep_in = parse_hex_u8(&self.ep_in_text)?;
        if ep_out & 0x80 != 0 {
            return Err(i18n::t("EP OUT 必须是 OUT 端点（bit 7 = 0，例如 04）").to_owned());
        }
        if ep_in & 0x80 == 0 {
            return Err(i18n::t("EP IN 必须是 IN 端点（bit 7 = 1，例如 84）").to_owned());
        }
        let interface = if self.interface_text.trim().is_empty() {
            None
        } else {
            Some(parse_hex_u8(&self.interface_text)?)
        };

        let mut config = DeviceConfig::new(vid, pid);
        config.sn = {
            let sn = self.sn_text.trim();
            if sn.is_empty() {
                None
            } else {
                Some(sn.to_owned())
            }
        };
        config.interface = interface;
        config.ep_out = ep_out;
        config.ep_in = ep_in;
        config.verbose = self.verbose;
        config.type2 = self.type2;
        config.log_sink = Some(self.worker.log_sink());
        Ok(config)
    }

    fn start(&mut self, task: Task, label: &str) {
        if self.busy {
            return;
        }
        let config = match self.build_config() {
            Ok(config) => config,
            Err(message) => {
                self.notify(message, Tone::Error);
                return;
            }
        };
        let label = i18n::t(label).to_owned();
        self.busy = true;
        self.busy_label = label.clone();
        self.log(format!("▶ {label}"));
        self.worker.send(Request::Run {
            config,
            task,
            label,
        });
    }

    fn confirm_write(&mut self) {
        let value = self
            .writelk_value
            .trim()
            .replace(' ', "")
            .to_ascii_lowercase();
        if value.len() != 8 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
            self.notify(
                i18n::t("锁值需要 8 位十六进制字符（4 字节，例如 80e48ed2）"),
                Tone::Error,
            );
            return;
        }
        self.confirm = Some(Confirm::WriteUim {
            value,
            control: self.writelk_control as u8,
        });
    }

    fn run_confirm(&mut self, confirm: Confirm) {
        match confirm {
            Confirm::ClearUim => self.start(Task::Clearlk, "清除 UIM 锁绑定"),
            Confirm::ResetPin => self.start(Task::Resetpin, "重置手机锁码"),
            Confirm::WriteUim { value, control } => {
                self.start(Task::Writelk { value, control }, "写入 UIM 锁绑定")
            }
            Confirm::MaintenanceBit { control } => {
                self.start(Task::MaintenanceBit { control }, "设置维护模式位")
            }
        }
    }

    fn start_probe(&mut self) {
        let timeout = self.probe_timeout.trim().parse::<f64>().unwrap_or(1.0);
        if !(timeout.is_finite() && timeout > 0.0) {
            self.notify(i18n::t("探测超时必须大于 0 秒"), Tone::Error);
            return;
        }
        let label = match self.probe_kind {
            ProbeKind::List => "USB 布局探测",
            ProbeKind::Seri => "SERI 回声探测",
            ProbeKind::Scdp => "SCDP 探测",
        };
        self.start(
            Task::Probe {
                kind: self.probe_kind,
                timeout_secs: timeout,
            },
            label,
        );
    }

    fn scan(&mut self, filtered: bool) {
        if self.busy {
            return;
        }
        let (vid, pid) = if filtered {
            match (parse_hex_u16(&self.vid_text), parse_hex_u16(&self.pid_text)) {
                (Ok(vid), Ok(pid)) => (Some(vid), Some(pid)),
                _ => {
                    self.notify(i18n::t("VID / PID 格式无效"), Tone::Error);
                    return;
                }
            }
        } else {
            (None, None)
        };
        self.scanning = true;
        self.worker.send(Request::Scan { vid, pid });
    }

    // --------------------------------------------------------------------------
    // eframe
    // --------------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        let mut open_about = false;
        let mut toggle_theme = false;
        let mut chosen_language: Option<Language> = None;

        ui.horizontal_centered(|ui| {
            let (logo, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
            ui.painter().rect_filled(logo, 13.0, s.primary_container);
            widgets::paint_icon(
                ui.painter(),
                Rect::from_center_size(logo.center(), vec2(24.0, 24.0)),
                Icon::Usb,
                s.on_primary_container,
            );
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("auRokku")
                        .size(21.0)
                        .strong()
                        .color(s.on_surface),
                );
                ui.label(
                    RichText::new(i18n::t("SERI / Type2Auth 设备工具"))
                        .size(11.5)
                        .color(s.on_surface_variant),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, &s, Icon::Info, "关于", true).clicked() {
                    open_about = true;
                }
                if icon_button(ui, &s, Icon::Contrast, "切换浅色 / 深色主题", true).clicked()
                {
                    toggle_theme = true;
                }

                egui::ComboBox::from_id_salt("auroku-language")
                    .width(96.0)
                    .selected_text(
                        RichText::new(self.lang.native_name())
                            .size(12.0)
                            .color(s.on_surface),
                    )
                    .show_ui(ui, |ui| {
                        let chinese = ui.add_enabled(
                            self.cjk_loaded,
                            egui::Button::selectable(self.lang == Language::ZhCn, "中文"),
                        );
                        if chinese.clicked() {
                            chosen_language = Some(Language::ZhCn);
                        }
                        if !self.cjk_loaded {
                            chinese.on_hover_text(i18n::t("未找到 CJK 字体，无法显示中文界面"));
                        }
                        if ui
                            .selectable_label(self.lang == Language::En, "English")
                            .clicked()
                        {
                            chosen_language = Some(Language::En);
                        }
                    });

                let (label, tone) = if self.busy {
                    ("执行中", Tone::Primary)
                } else if self.last_error.is_some() {
                    ("错误", Tone::Error)
                } else if self.last_success == Some(true) {
                    ("就绪", Tone::Success)
                } else if self.last_success == Some(false) {
                    ("失败", Tone::Warning)
                } else {
                    ("待命", Tone::Neutral)
                };
                chip(ui, &s, label, tone);
                chip(
                    ui,
                    &s,
                    &format!("{}:{}", self.vid_text, self.pid_text),
                    Tone::Neutral,
                );

                if self.busy {
                    ui.spinner();
                    ui.label(
                        RichText::new(&self.busy_label)
                            .size(12.0)
                            .color(s.on_surface_variant),
                    );
                }
            });
        });

        if open_about {
            self.show_about = true;
        }
        if toggle_theme {
            self.dark = !self.dark;
        }
        if let Some(language) = chosen_language {
            self.set_language(language);
        }
    }

    fn set_language(&mut self, language: Language) {
        if self.lang == language {
            return;
        }
        self.lang = language;
        i18n::set(language);
        let name = language.native_name();
        self.notify(
            i18n::tf("界面语言已切换为 {lang}", &[("lang", name)]),
            Tone::Primary,
        );
    }

    fn footer(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(i18n::tf(
                    "auRokku v{version} · 基于Yuu所提供的代码二次开发 ©lazybri",
                    &[("version", env!("CARGO_PKG_VERSION"))],
                ))
                .size(11.0)
                .color(s.on_surface_variant),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let permission = if cfg!(target_os = "linux") {
                    if crate::elevate::is_effective_root() {
                        i18n::t("权限：root")
                    } else {
                        i18n::t("权限：普通用户 · USB 访问可能受限")
                    }
                } else {
                    i18n::t("USB 访问可能需要管理员权限")
                };
                ui.label(
                    RichText::new(permission)
                        .size(11.0)
                        .color(s.on_surface_variant),
                );
            });
        });
    }

    fn nav_rail(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        ui.vertical(|ui| {
            for screen in Screen::ALL {
                let selected = self.screen == screen;
                let (rect, response) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 66.0), Sense::click());
                if response.clicked() {
                    self.screen = screen;
                }
                let pill = Rect::from_center_size(
                    pos2(rect.center().x, rect.top() + 24.0),
                    vec2(58.0, 32.0),
                );
                let painter = ui.painter();
                if selected {
                    painter.rect_filled(pill, 16.0, s.secondary_container);
                } else if response.hovered() {
                    painter.rect_filled(pill, 16.0, s.surface_container_high);
                }
                widgets::paint_icon(
                    painter,
                    Rect::from_center_size(pill.center(), vec2(20.0, 20.0)),
                    screen.icon(),
                    if selected {
                        s.on_secondary_container
                    } else {
                        s.on_surface_variant
                    },
                );
                painter.text(
                    pos2(rect.center().x, rect.bottom() - 13.0),
                    Align2::CENTER_CENTER,
                    screen.label(),
                    FontId::proportional(11.0),
                    if selected {
                        s.on_surface
                    } else {
                        s.on_surface_variant
                    },
                );
            }
        });
    }

    fn content(&mut self, ui: &mut Ui) {
        match self.screen {
            Screen::Overview => self.page_overview(ui),
            Screen::Uim => self.page_uim(ui),
            Screen::Pin => self.page_pin(ui),
            Screen::Probe => self.page_probe(ui),
            Screen::Batch => self.page_batch(ui),
            Screen::Session => self.page_session(ui),
            Screen::Console => self.page_console(ui),
        }
    }

    // --------------------------------------------------------------------------
    // Overview
    // --------------------------------------------------------------------------

    fn page_overview(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(
            ui,
            &s,
            "概览",
            "连接 au / SERI 设备，读取锁状态、PIN 与 SIM 信息",
        );

        if cfg!(target_os = "linux") && !crate::elevate::is_effective_root() {
            banner(
                ui,
                &s,
                Icon::Warning,
                "当前以普通用户运行，USB 访问可能失败。可使用 pkexec 重新启动并完成 polkit 认证，或配置 udev 规则。",
                Tone::Warning,
            );
            ui.add_space(12.0);
        }

        let devices = self.devices.clone();
        let current_device = self
            .selected_device
            .and_then(|index| devices.get(index))
            .map(|device| format!("{} · {}", device.display_name(), device.id_string()))
            .unwrap_or_else(|| i18n::t("选择已扫描设备…").to_owned());

        card(ui, &s, |ui| {
            card_header(ui, &s, Icon::Usb, "设备连接", "扫描 USB 总线并选择目标设备");

            let mut picked = None;
            ui.horizontal(|ui| {
                if button(
                    ui,
                    &s,
                    Some(Icon::Scan),
                    if self.scanning {
                        "扫描中…"
                    } else {
                        "扫描全部"
                    },
                    ButtonKind::Tonal,
                    !self.busy && !self.scanning,
                    0.0,
                )
                .clicked()
                {
                    self.scan(false);
                }
                egui::ComboBox::from_id_salt("device-picker")
                    .width(380.0)
                    .selected_text(current_device)
                    .show_ui(ui, |ui| {
                        if devices.is_empty() {
                            ui.label(i18n::t("未发现设备（请检查 USB 权限后重新扫描）"));
                        }
                        for (index, device) in devices.iter().enumerate() {
                            let text =
                                format!("{}  ·  {}", device.display_name(), device.id_string());
                            if ui
                                .selectable_label(self.selected_device == Some(index), text)
                                .clicked()
                            {
                                picked = Some(index);
                            }
                        }
                    });
            });
            if let Some(index) = picked {
                self.apply_device(index, true);
            }

            if let Some(error) = &self.scan_error {
                ui.add_space(6.0);
                banner(ui, &s, Icon::Warning, error, Tone::Error);
            }

            ui.add_space(10.0);
            egui::Grid::new("device-fields")
                .num_columns(3)
                .spacing([14.0, 10.0])
                .show(ui, |ui| {
                    text_field(
                        ui,
                        &s,
                        "VID (hex)",
                        "0482",
                        &mut self.vid_text,
                        150.0,
                        true,
                        true,
                    );
                    text_field(
                        ui,
                        &s,
                        "PID (hex)",
                        "0a5c",
                        &mut self.pid_text,
                        150.0,
                        true,
                        true,
                    );
                    text_field(
                        ui,
                        &s,
                        "SN（可选）",
                        "SKYKA077394 或 16 位 hex",
                        &mut self.sn_text,
                        260.0,
                        true,
                        true,
                    );
                    ui.end_row();

                    text_field(
                        ui,
                        &s,
                        "接口号（可选）",
                        "自动",
                        &mut self.interface_text,
                        150.0,
                        true,
                        true,
                    );
                    text_field(
                        ui,
                        &s,
                        "EP OUT",
                        "04",
                        &mut self.ep_out_text,
                        150.0,
                        true,
                        true,
                    );
                    text_field(
                        ui,
                        &s,
                        "EP IN",
                        "84",
                        &mut self.ep_in_text,
                        150.0,
                        true,
                        true,
                    );
                    ui.end_row();
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                switch(ui, &s, &mut self.type2);
                ui.label(
                    RichText::new(i18n::t("Type2Auth 52/53 认证"))
                        .size(13.0)
                        .color(s.on_surface),
                );
                ui.add_space(16.0);
                switch(ui, &s, &mut self.verbose);
                ui.label(
                    RichText::new(i18n::t("详细 USB 日志"))
                        .size(13.0)
                        .color(s.on_surface),
                );
            });

            ui.add_space(6.0);
            banner(
                ui,
                &s,
                Icon::Info,
                "SN 留空时读取设备 iSerialNumber；若设备不提供序列号，请手动填写 11 位 SN 或 16 位十六进制。",
                Tone::Neutral,
            );
        });

        ui.add_space(12.0);

        let enabled = !self.busy && !self.scanning;
        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Play,
                "快捷操作",
                "所有 USB 操作都在后台线程执行，界面不会卡住",
            );
            ui.horizontal_wrapped(|ui| {
                if button(
                    ui,
                    &s,
                    Some(Icon::Dashboard),
                    "读取综合状态",
                    ButtonKind::Filled,
                    enabled,
                    0.0,
                )
                .clicked()
                {
                    self.start(Task::Status, "读取综合状态");
                }
                if button(
                    ui,
                    &s,
                    Some(Icon::Probe),
                    "USB 布局探测",
                    ButtonKind::Tonal,
                    enabled,
                    0.0,
                )
                .clicked()
                {
                    self.probe_kind = ProbeKind::List;
                    self.start_probe();
                }
                if button(
                    ui,
                    &s,
                    Some(Icon::Power),
                    "退出服务 (0xD8)",
                    ButtonKind::Outlined,
                    enabled,
                    0.0,
                )
                .clicked()
                {
                    self.start(Task::Exit, "退出服务");
                }
                if button(
                    ui,
                    &s,
                    Some(Icon::Terminal),
                    "打开日志控制台",
                    ButtonKind::Text,
                    true,
                    0.0,
                )
                .clicked()
                {
                    self.screen = Screen::Console;
                }
            });
            if self.busy {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new(i18n::tf(
                            "正在执行：{label}",
                            &[("label", &self.busy_label)],
                        ))
                        .size(12.5)
                        .color(s.on_surface_variant),
                    );
                });
            }
        });

        ui.add_space(12.0);
        self.last_result_card(ui);
    }

    fn last_result_card(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        let result = self.result.clone();
        let error = self.last_error.clone();
        let op = self.last_op.clone();
        let success = self.last_success;
        let mut open_json = false;

        card(ui, &s, |ui| {
            card_header(ui, &s, Icon::Info, "最近结果", "最近一次操作的结构化摘要");

            if let Some(error) = &error {
                banner(
                    ui,
                    &s,
                    Icon::Warning,
                    &format!("{}: {}", error.kind, error.message),
                    Tone::Error,
                );
            } else if let Some(result) = &result {
                ui.horizontal(|ui| {
                    chip(
                        ui,
                        &s,
                        if success == Some(true) {
                            "成功"
                        } else {
                            "失败"
                        },
                        if success == Some(true) {
                            Tone::Success
                        } else {
                            Tone::Warning
                        },
                    );
                    if let Some(op) = &op {
                        ui.label(
                            RichText::new(op)
                                .monospace()
                                .size(12.5)
                                .color(s.on_surface_variant),
                        );
                    }
                });
                ui.add_space(8.0);

                if success == Some(true)
                    && result.get("operation").and_then(Value::as_str) == Some("status")
                {
                    self.status_summary(ui, result);
                } else {
                    self.generic_result_summary(ui, result);
                }

                ui.add_space(8.0);
                if button(
                    ui,
                    &s,
                    Some(Icon::Terminal),
                    "查看完整 JSON",
                    ButtonKind::Text,
                    true,
                    0.0,
                )
                .clicked()
                {
                    open_json = true;
                }
            } else {
                empty_state(
                    ui,
                    &s,
                    Icon::Info,
                    "还没有结果",
                    "运行一次状态读取或探测后，结果会显示在这里",
                );
            }
        });

        if open_json {
            self.screen = Screen::Console;
        }
    }

    fn status_summary(&self, ui: &mut Ui, result: &Value) {
        let s = self.scheme.clone();
        let sn = value_str(result, "sn").unwrap_or_else(|| "N/A".to_owned());
        let pin = value_str(result, "lockcode").unwrap_or_else(|| "N/A".to_owned());
        let version = value_str(result, "version").unwrap_or_else(|| "N/A".to_owned());
        let iccid = value_str(result, "iccid_record").unwrap_or_else(|| "N/A".to_owned());

        let uim = result.get("uim_lock");
        let unlocked = uim
            .and_then(|value| value.get("unlocked"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let lock_value = uim
            .and_then(|value| value.get("lock_value"))
            .and_then(Value::as_str)
            .unwrap_or("N/A");
        let uim_text = if unlocked {
            i18n::t("已解锁 (ffffffff)").to_owned()
        } else {
            i18n::tf("已锁定 ({value})", &[("value", lock_value)])
        };

        ui.columns(2, |cols| {
            metric(&mut cols[0], &s, Icon::Usb, "设备 SN", &sn, Tone::Primary);
            metric(
                &mut cols[1],
                &s,
                Icon::Key,
                "手机 PIN",
                &pin,
                Tone::Tertiary,
            );
        });
        ui.add_space(8.0);
        ui.columns(2, |cols| {
            metric(
                &mut cols[0],
                &s,
                Icon::Lock,
                "UIM 锁",
                &uim_text,
                if unlocked {
                    Tone::Success
                } else {
                    Tone::Warning
                },
            );
            metric(&mut cols[1], &s, Icon::List, "ICCID", &iccid, Tone::Neutral);
        });
        ui.add_space(8.0);
        kv_row(ui, &s, "Base 版本 (raw)", &version, true);
    }

    fn generic_result_summary(&self, ui: &mut Ui, result: &Value) {
        let s = self.scheme.clone();
        let keys = [
            ("lock_value", "UIM 锁值"),
            ("control", "控制位"),
            ("lockcode", "手机 PIN"),
            ("decoded_hex", "解码数据"),
            ("raw_hex", "原始数据"),
            ("reply", "应答"),
            ("request_hex", "请求"),
            ("response_hex", "响应"),
            ("maintenance_bit", "维护位"),
            ("method", "退出方式"),
        ];
        let mut shown = 0;
        for (key, label) in keys {
            if let Some(value) = result.get(key) {
                let text = py_value(value);
                if !text.is_empty() && text != "None" {
                    kv_row(ui, &s, label, &text, true);
                    shown += 1;
                }
            }
        }
        if let Some(verified) = result.get("verified").and_then(Value::as_bool) {
            kv_row(
                ui,
                &s,
                "写入校验",
                if verified { "通过" } else { "未通过" },
                false,
            );
            shown += 1;
        }
        if let Some(accepted) = result.get("command_accepted").and_then(Value::as_bool) {
            kv_row(
                ui,
                &s,
                "命令接受",
                if accepted { "是" } else { "否" },
                false,
            );
            shown += 1;
        }
        if shown == 0 {
            subtle_label(ui, &s, "该操作没有可展开的摘要字段，请查看完整 JSON。");
        }
    }

    // --------------------------------------------------------------------------
    // UIM lock
    // --------------------------------------------------------------------------

    fn page_uim(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(
            ui,
            &s,
            "UIM 锁",
            "读取、写入或清除 SIM 锁绑定（0x54 / 0x72 / 0x57）",
        );

        banner(
            ui,
            &s,
            Icon::Warning,
            "写入或清除锁会修改 SIM 绑定状态。执行前请确认设备与卡片匹配，避免写入错误绑定值。",
            Tone::Warning,
        );
        ui.add_space(12.0);

        let enabled = !self.busy && !self.scanning;

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Lock,
                "读取锁状态",
                "进入 UIMLK 会话并读取 control 位与绑定值",
            );
            if button(
                ui,
                &s,
                Some(Icon::Scan),
                "读取 UIM 锁",
                ButtonKind::Filled,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.start(Task::ReadUimlk, "读取 UIM 锁");
            }
            if let Some(result) = self.result.clone() {
                if result.get("operation").and_then(Value::as_str) == Some("readuimlk") {
                    ui.add_space(8.0);
                    self.uim_result(ui, &result);
                }
            }
        });

        ui.add_space(12.0);

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Write,
                "写入绑定值",
                "0x72：写 4 字节锁值并校验回读",
            );
            ui.horizontal(|ui| {
                text_field(
                    ui,
                    &s,
                    "锁值（8 位 hex）",
                    "80e48ed2",
                    &mut self.writelk_value,
                    220.0,
                    true,
                    enabled,
                );
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(i18n::t("控制位"))
                            .size(11.5)
                            .color(s.on_surface_variant),
                    );
                    ui.horizontal(|ui| {
                        switch(ui, &s, &mut self.writelk_control);
                        ui.label(
                            RichText::new(if self.writelk_control { "1" } else { "0" })
                                .monospace()
                                .size(13.0)
                                .color(s.on_surface),
                        );
                    });
                });
            });
            ui.add_space(8.0);
            if button(
                ui,
                &s,
                Some(Icon::Write),
                "写入绑定值",
                ButtonKind::Tonal,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.confirm_write();
            }
        });

        ui.add_space(12.0);

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Trash,
                "清除绑定",
                "0x57：把锁值写为 ffffffff（解除绑定）",
            );
            if button(
                ui,
                &s,
                Some(Icon::Trash),
                "清除 UIM 锁绑定",
                ButtonKind::DangerOutlined,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.confirm = Some(Confirm::ClearUim);
            }
            ui.add_space(6.0);
            subtle_label(ui, &s, "命令接受后会回读锁值并校验是否为 ffffffff。");
        });

        ui.add_space(12.0);

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Power,
                "维护模式位 (NV 10481)",
                "0xE1 服务 + auth 4000 认证后写入",
            );
            ui.horizontal(|ui| {
                switch(ui, &s, &mut self.maint_control);
                ui.label(
                    RichText::new(format!(
                        "control = {}",
                        if self.maint_control { "1" } else { "0" }
                    ))
                    .monospace()
                    .size(13.0)
                    .color(s.on_surface),
                );
                ui.add_space(12.0);
                if button(
                    ui,
                    &s,
                    Some(Icon::Play),
                    "应用维护位",
                    ButtonKind::Outlined,
                    enabled,
                    0.0,
                )
                .clicked()
                {
                    self.confirm = Some(Confirm::MaintenanceBit {
                        control: self.maint_control as u8,
                    });
                }
            });
            ui.add_space(6.0);
            subtle_label(
                ui,
                &s,
                "需要有效的 SN / Type2Auth 材料；认证失败时会返回 0x53 校验错误。",
            );
        });

        ui.add_space(12.0);
        self.last_result_card(ui);
    }

    fn uim_result(&self, ui: &mut Ui, result: &Value) {
        let s = self.scheme.clone();
        let value = value_str(result, "lock_value").unwrap_or_default();
        let control = value_str(result, "control").unwrap_or_default();
        let cleared = result
            .get("binding_cleared")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        ui.horizontal(|ui| {
            chip(
                ui,
                &s,
                if cleared {
                    "已解锁 / 已清除"
                } else {
                    "已锁定"
                },
                if cleared {
                    Tone::Success
                } else {
                    Tone::Warning
                },
            );
            chip(ui, &s, &format!("control = {control}"), Tone::Neutral);
        });
        ui.add_space(6.0);
        kv_row(ui, &s, "锁值", &value, true);
        if let Some(raw) = value_str(result, "raw_hex") {
            kv_row(ui, &s, "原始应答", &raw, true);
        }

        if let Some(before) = result.get("before") {
            key_value(ui, &s, "写入前", before, "lock_value");
        }
        if let Some(after) = result.get("after") {
            key_value(ui, &s, "写入后", after, "lock_value");
        }
        if let Some(verified) = result.get("verified").and_then(Value::as_bool) {
            kv_row(
                ui,
                &s,
                "回读校验",
                if verified { "通过" } else { "未通过" },
                false,
            );
        }
    }

    // --------------------------------------------------------------------------
    // PIN / ICCID
    // --------------------------------------------------------------------------

    fn page_pin(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(
            ui,
            &s,
            "PIN / ICCID",
            "读取或重置手机锁码（NV 82），读取 ICCID record 0x59",
        );

        let enabled = !self.busy && !self.scanning;

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Key,
                "读取手机锁码",
                "SERI 0xDC 0x00，按 0xAA 终止符解析",
            );
            if button(
                ui,
                &s,
                Some(Icon::Scan),
                "读取 PIN",
                ButtonKind::Filled,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.start(Task::Readpin, "读取手机锁码");
            }

            if let Some(result) = self.result.clone() {
                if result.get("operation").and_then(Value::as_str) == Some("readpin") {
                    ui.add_space(8.0);
                    let pin = value_str(&result, "lockcode").unwrap_or_else(|| "N/A".to_owned());
                    metric(ui, &s, Icon::Key, "手机锁码", &pin, Tone::Tertiary);
                    if let Some(raw) = value_str(&result, "raw_hex") {
                        kv_row(ui, &s, "原始应答", &raw, true);
                    }
                }
            }
        });

        ui.add_space(12.0);

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Refresh,
                "重置手机锁码",
                "SERI 0xDC 0x01，恢复默认值 1234 并回读校验",
            );
            if button(
                ui,
                &s,
                Some(Icon::Refresh),
                "重置 PIN 为 1234",
                ButtonKind::DangerOutlined,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.confirm = Some(Confirm::ResetPin);
            }

            if let Some(result) = self.result.clone() {
                if result.get("operation").and_then(Value::as_str) == Some("resetpin") {
                    ui.add_space(8.0);
                    let pin = value_str(&result, "lockcode").unwrap_or_else(|| "N/A".to_owned());
                    let verified = result
                        .get("verified")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    ui.horizontal(|ui| {
                        metric(
                            ui,
                            &s,
                            Icon::Key,
                            "回读 PIN",
                            &pin,
                            if verified {
                                Tone::Success
                            } else {
                                Tone::Warning
                            },
                        );
                    });
                    kv_row(
                        ui,
                        &s,
                        "1234 校验",
                        if verified { "通过" } else { "未通过" },
                        false,
                    );
                }
            }
        });

        ui.add_space(12.0);

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::List,
                "ICCID 记录",
                "进入 UIMLK 会话读取 record 0x59",
            );
            if button(
                ui,
                &s,
                Some(Icon::Scan),
                "读取 ICCID",
                ButtonKind::Tonal,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.start(Task::ReadIccid, "读取 ICCID");
            }

            if let Some(result) = self.result.clone() {
                if result.get("operation").and_then(Value::as_str) == Some("readiccid") {
                    ui.add_space(8.0);
                    let decoded =
                        value_str(&result, "decoded_hex").unwrap_or_else(|| "N/A".to_owned());
                    metric(ui, &s, Icon::List, "解码 ICCID", &decoded, Tone::Primary);
                    if let Some(raw) = value_str(&result, "raw_hex") {
                        kv_row(ui, &s, "原始应答", &raw, true);
                    }
                }
            }
        });

        ui.add_space(12.0);
        self.last_result_card(ui);
    }

    // --------------------------------------------------------------------------
    // Probe
    // --------------------------------------------------------------------------

    fn page_probe(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(ui, &s, "探测", "枚举 USB 布局，或主动探测 SERI / SCDP 服务");

        let enabled = !self.busy && !self.scanning;

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Probe,
                "探测模式",
                "list 只枚举配置；seri / scdp 会实际收发 USB 数据",
            );
            segmented(
                ui,
                &s,
                &[
                    (ProbeKind::List, "布局 list"),
                    (ProbeKind::Seri, "SERI 回声"),
                    (ProbeKind::Scdp, "SCDP"),
                ],
                &mut self.probe_kind,
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                text_field(
                    ui,
                    &s,
                    "探测超时（秒）",
                    "1.0",
                    &mut self.probe_timeout,
                    160.0,
                    true,
                    enabled && self.probe_kind != ProbeKind::List,
                );
                ui.add_space(12.0);
                if button(
                    ui,
                    &s,
                    Some(Icon::Play),
                    "开始探测",
                    ButtonKind::Filled,
                    enabled,
                    0.0,
                )
                .clicked()
                {
                    self.start_probe();
                }
            });
            ui.add_space(6.0);
            subtle_label(
                ui,
                &s,
                "SERI 探测会发送 1C 05；非 0FCE 设备在成功后自动发送 D8 清理。",
            );
        });

        ui.add_space(12.0);

        let result = self.result.clone();
        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Usb,
                "USB 布局",
                "配置 / 接口 / 端点与候选 bulk 端点对",
            );
            match result.as_ref() {
                Some(value) if value.get("operation").and_then(Value::as_str) == Some("probe") => {
                    self.probe_result(ui, value);
                }
                _ => empty_state(
                    ui,
                    &s,
                    Icon::Usb,
                    "尚无探测结果",
                    "运行布局探测后，这里会显示 USB 配置树与端点信息",
                ),
            }
        });

        ui.add_space(12.0);
        self.last_result_card(ui);
    }

    fn probe_result(&self, ui: &mut Ui, result: &Value) {
        let s = self.scheme.clone();

        let kind = value_str(result, "probe").unwrap_or_default();
        ui.horizontal(|ui| {
            chip(ui, &s, &format!("probe = {kind}"), Tone::Primary);
            if result
                .get("matched")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                chip(ui, &s, "SERI 回声匹配", Tone::Success);
            }
            if result
                .get("responsive")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                chip(ui, &s, "设备有响应", Tone::Success);
            }
        });

        if let Some(selection) = result.get("selection") {
            let interface = selection.get("interface").map(py_value).unwrap_or_default();
            if selection.is_object() && interface != "None" && !interface.is_empty() {
                ui.add_space(6.0);
                let alt = value_str(selection, "alternate_setting").unwrap_or_default();
                let ep_out = value_str(selection, "ep_out").unwrap_or_default();
                let ep_in = value_str(selection, "ep_in").unwrap_or_default();
                kv_row(
                    ui,
                    &s,
                    "选中接口",
                    &format!("if {interface} alt {alt} · OUT {ep_out} / IN {ep_in}"),
                    true,
                );
            }
        }

        if let Some(request) = value_str(result, "request_hex") {
            kv_row(ui, &s, "请求", &request, true);
        }
        if let Some(response) = value_str(result, "response_hex") {
            kv_row(
                ui,
                &s,
                "响应",
                if response.is_empty() {
                    "<超时>"
                } else {
                    &response
                },
                true,
            );
        }
        if let Some(cleanup) = value_str(result, "cleanup_hex") {
            kv_row(ui, &s, "清理命令", &cleanup, true);
        }

        ui.add_space(8.0);
        if let Some(usb) = result.get("usb") {
            let vid = value_str(usb, "vid").unwrap_or_default();
            let pid = value_str(usb, "pid").unwrap_or_default();
            subtle_label(
                ui,
                &s,
                &i18n::tf("设备 {vid}:{pid}", &[("vid", &vid), ("pid", &pid)]),
            );

            if let Some(configurations) = usb.get("configurations").and_then(Value::as_array) {
                for config in configurations {
                    let value = value_str(config, "value").unwrap_or_default();
                    let active = config
                        .get("active")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    ui.horizontal(|ui| {
                        chip(
                            ui,
                            &s,
                            &i18n::tf("配置 {value}", &[("value", &value)]),
                            if active { Tone::Primary } else { Tone::Neutral },
                        );
                        if active {
                            chip(ui, &s, "活动", Tone::Success);
                        }
                    });
                    if let Some(interfaces) = config.get("interfaces").and_then(Value::as_array) {
                        for interface in interfaces {
                            let number = value_str(interface, "number").unwrap_or_default();
                            let alt = value_str(interface, "alternate_setting").unwrap_or_default();
                            let class = value_str(interface, "class").unwrap_or_default();
                            let subclass = value_str(interface, "subclass").unwrap_or_default();
                            let protocol = value_str(interface, "protocol").unwrap_or_default();
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(i18n::tf(
                                        "接口 {number} · alt {alt}",
                                        &[("number", &number), ("alt", &alt)],
                                    ))
                                    .size(12.5)
                                    .strong()
                                    .color(s.on_surface),
                                );
                                ui.label(
                                    RichText::new(format!("class {class}/{subclass}/{protocol}"))
                                        .monospace()
                                        .size(11.5)
                                        .color(s.on_surface_variant),
                                );
                            });
                            if let Some(endpoints) =
                                interface.get("endpoints").and_then(Value::as_array)
                            {
                                for endpoint in endpoints {
                                    let address =
                                        value_str(endpoint, "address").unwrap_or_default();
                                    let direction =
                                        value_str(endpoint, "direction").unwrap_or_default();
                                    let transfer = value_str(endpoint, "type").unwrap_or_default();
                                    let packet =
                                        value_str(endpoint, "max_packet_size").unwrap_or_default();
                                    ui.label(
                                        RichText::new(format!(
                                            "    {address} {direction} · {transfer} · {} B",
                                            packet
                                        ))
                                        .monospace()
                                        .size(11.5)
                                        .color(s.on_surface_variant),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            if let Some(candidates) = usb.get("bulk_candidates").and_then(Value::as_array) {
                if candidates.is_empty() {
                    subtle_label(ui, &s, "未发现 bulk 端点对");
                } else {
                    ui.add_space(4.0);
                    subtle_label(ui, &s, "候选 bulk 端点对");
                    for candidate in candidates {
                        let config = value_str(candidate, "configuration").unwrap_or_default();
                        let interface = value_str(candidate, "interface").unwrap_or_default();
                        let alt = value_str(candidate, "alternate_setting").unwrap_or_default();
                        let ep_out = value_str(candidate, "ep_out").unwrap_or_default();
                        let ep_in = value_str(candidate, "ep_in").unwrap_or_default();
                        let active = candidate
                            .get("active")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "cfg {config} · if {interface} alt {alt} · OUT {ep_out} / IN {ep_in}"
                                ))
                                .monospace()
                                .size(11.5)
                                .color(s.on_surface),
                            );
                            if active {
                                chip(ui, &s, "活动", Tone::Success);
                            }
                        });
                    }
                }
            }
        }
    }

    // --------------------------------------------------------------------------
    // Batch
    // --------------------------------------------------------------------------

    fn page_batch(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(
            ui,
            &s,
            "批量查询",
            "在单个会话内连续执行多条查询，避免反复进入 / 退出服务",
        );

        let enabled = !self.busy && !self.scanning;

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::List,
                "批量配置",
                "basic：版本 + 锁码；uimlk：锁状态 + record59",
            );
            segmented(
                ui,
                &s,
                &[
                    (BatchProfile::Basic, "基础 basic"),
                    (BatchProfile::Uimlk, "UIMLK uimlk"),
                ],
                &mut self.batch_profile,
            );
            ui.add_space(8.0);
            if button(
                ui,
                &s,
                Some(Icon::Play),
                "执行批量查询",
                ButtonKind::Filled,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.start(
                    Task::Batch {
                        profile: self.batch_profile,
                    },
                    "批量查询",
                );
            }
            ui.add_space(6.0);
            subtle_label(ui, &s, "uimlk profile 需要先完成 UIMLK 认证，SN 必须可用。");
        });

        ui.add_space(12.0);

        let result = self.result.clone();
        card(ui, &s, |ui| {
            card_header(ui, &s, Icon::List, "查询结果", "每条查询的原始字段与解码值");
            match result.as_ref() {
                Some(value) if value.get("operation").and_then(Value::as_str) == Some("batch") => {
                    let profile = value_str(value, "profile").unwrap_or_default();
                    ui.horizontal(|ui| {
                        chip(ui, &s, &format!("profile = {profile}"), Tone::Primary);
                    });
                    if let Some(error) = value.get("error") {
                        banner(
                            ui,
                            &s,
                            Icon::Warning,
                            &value_str(error, "message").unwrap_or_default(),
                            Tone::Error,
                        );
                    }
                    if let Some(results) = value.get("results").and_then(Value::as_array) {
                        for item in results {
                            ui.add_space(6.0);
                            let query = value_str(item, "query").unwrap_or_default();
                            ui.label(
                                RichText::new(i18n::tf("查询：{query}", &[("query", &query)]))
                                    .size(13.0)
                                    .strong()
                                    .color(s.on_surface),
                            );
                            for key in ["decoded", "lock_value", "raw_hex", "decoded_hex"] {
                                if let Some(field) = item.get(key) {
                                    kv_row(ui, &s, key, &py_value(field), true);
                                }
                            }
                        }
                    }
                }
                _ => empty_state(
                    ui,
                    &s,
                    Icon::List,
                    "尚无批量结果",
                    "选择 profile 并执行查询后，这里会列出每条结果",
                ),
            }
        });

        ui.add_space(12.0);
        self.last_result_card(ui);
    }

    // --------------------------------------------------------------------------
    // Session
    // --------------------------------------------------------------------------

    fn page_session(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(
            ui,
            &s,
            "会话与服务",
            "退出维护 / UIM 服务，让设备回到正常模式",
        );

        let enabled = !self.busy && !self.scanning;

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Power,
                "退出基础服务",
                "发送 0xD8，恢复手机键盘 / 正常界面",
            );
            if button(
                ui,
                &s,
                Some(Icon::Power),
                "退出服务 (0xD8)",
                ButtonKind::Outlined,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.start(Task::Exit, "退出服务");
            }
            subtle_label(ui, &s, "对 0FCE 设备不会发送 0xD8（与 Python 版一致）。");
        });

        ui.add_space(12.0);

        card(ui, &s, |ui| {
            card_header(
                ui,
                &s,
                Icon::Stop,
                "退出 UIMLK 子服务",
                "重新进入 UIMLK 并发送 0x56，设备可能重启",
            );
            if button(
                ui,
                &s,
                Some(Icon::Stop),
                "退出 UIMLK (0x56)",
                ButtonKind::DangerOutlined,
                enabled,
                0.0,
            )
            .clicked()
            {
                self.start(Task::Exituimlk, "退出 UIMLK");
            }
            ui.add_space(6.0);
            subtle_label(
                ui,
                &s,
                "返回结果中的 exit 字段会记录 acknowledged / reply / may_reboot。",
            );
        });

        ui.add_space(12.0);

        let result = self.result.clone();
        card(ui, &s, |ui| {
            card_header(ui, &s, Icon::Info, "会话详情", "退出应答与异步事件");
            match result.as_ref() {
                Some(value) => {
                    if let Some(exit) = value.get("exit").filter(|value| value.is_object()) {
                        let acknowledged = exit
                            .get("acknowledged")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        ui.horizontal(|ui| {
                            chip(
                                ui,
                                &s,
                                if acknowledged {
                                    "退出已确认"
                                } else {
                                    "退出未确认"
                                },
                                if acknowledged {
                                    Tone::Success
                                } else {
                                    Tone::Warning
                                },
                            );
                            if exit
                                .get("may_reboot")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                            {
                                chip(ui, &s, "可能重启", Tone::Warning);
                            }
                        });
                        if let Some(reply) = value_str(exit, "reply") {
                            kv_row(ui, &s, "应答", &reply, true);
                        }
                        if let Some(error) = value_str(exit, "error") {
                            kv_row(ui, &s, "错误", &error, false);
                        }
                    } else {
                        subtle_label(ui, &s, "本次操作没有产生服务退出记录。");
                    }

                    if let Some(events) = value.get("async_events").and_then(Value::as_array) {
                        if !events.is_empty() {
                            ui.add_space(6.0);
                            subtle_label(ui, &s, "异步事件");
                            for event in events {
                                let opcode = value_str(event, "opcode").unwrap_or_default();
                                let meaning = value_str(event, "meaning").unwrap_or_default();
                                kv_row(ui, &s, &format!("opcode {opcode}"), &meaning, true);
                            }
                        }
                    }
                }
                None => empty_state(
                    ui,
                    &s,
                    Icon::Power,
                    "还没有会话记录",
                    "执行退出服务后，应答信息会显示在这里",
                ),
            }
        });

        ui.add_space(12.0);
        self.last_result_card(ui);
    }

    // --------------------------------------------------------------------------
    // Console
    // --------------------------------------------------------------------------

    fn page_console(&mut self, ui: &mut Ui) {
        let s = self.scheme.clone();
        page_header(ui, &s, "控制台", "USB 收发日志与最近一次操作的完整 JSON");

        let mut clear = false;
        card(ui, &s, |ui| {
            ui.horizontal(|ui| {
                switch(ui, &s, &mut self.verbose);
                ui.label(
                    RichText::new(i18n::t("详细 USB 日志"))
                        .size(13.0)
                        .color(s.on_surface),
                );
                ui.add_space(12.0);
                switch(ui, &s, &mut self.auto_scroll_logs);
                ui.label(
                    RichText::new(i18n::t("自动滚动"))
                        .size(13.0)
                        .color(s.on_surface),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(
                        ui,
                        &s,
                        Some(Icon::Trash),
                        "清空日志",
                        ButtonKind::Text,
                        true,
                        0.0,
                    )
                    .clicked()
                    {
                        clear = true;
                    }
                    chip(
                        ui,
                        &s,
                        &i18n::tf("{count} 条", &[("count", &self.logs.len().to_string())]),
                        Tone::Neutral,
                    );
                });
            });
            ui.add_space(8.0);
            Frame::new()
                .fill(s.surface_container_lowest)
                .stroke(Stroke::new(1.0_f32, s.outline_variant))
                .corner_radius(14.0)
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ScrollArea::vertical()
                        .max_height(260.0)
                        .stick_to_bottom(self.auto_scroll_logs)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if self.logs.is_empty() {
                                subtle_label(
                                    ui,
                                    &s,
                                    "暂无日志。启用“详细 USB 日志”可看到 TX / RX 帧。",
                                );
                            }
                            for line in &self.logs {
                                let color = if line.text.starts_with("TX->") {
                                    s.primary
                                } else if line.text.starts_with("RX->") {
                                    s.tertiary
                                } else if line.text.contains("[WARN]") {
                                    s.error
                                } else if line.text.contains("[INFO]") {
                                    s.on_surface_variant
                                } else {
                                    s.on_surface
                                };
                                ui.label(
                                    RichText::new(format!("{:>8.2}s  {}", line.at, line.text))
                                        .monospace()
                                        .size(11.5)
                                        .color(color),
                                );
                            }
                        });
                });
        });

        if clear {
            self.logs.clear();
            self.log(i18n::t("日志已清空").to_owned());
        }

        ui.add_space(12.0);

        let pretty = self.pretty_json.clone();
        let has_result = self.result.is_some();
        let mut copy = false;
        card(ui, &s, |ui| {
            ui.horizontal(|ui| {
                card_header(
                    ui,
                    &s,
                    Icon::Terminal,
                    "完整 JSON",
                    self.last_op.as_deref().unwrap_or("最近一次操作"),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(
                        ui,
                        &s,
                        Some(Icon::Copy),
                        "复制",
                        ButtonKind::Text,
                        has_result,
                        0.0,
                    )
                    .clicked()
                    {
                        copy = true;
                    }
                });
            });
            if has_result {
                code_block(ui, &s, &pretty, 380.0);
            } else {
                empty_state(
                    ui,
                    &s,
                    Icon::Terminal,
                    "没有 JSON 结果",
                    "执行一次操作后，完整协议结果会显示在这里",
                );
            }
        });
        if copy {
            ui.ctx().copy_text(pretty);
            self.notify(i18n::t("JSON 已复制到剪贴板"), Tone::Success);
        }
    }

    // --------------------------------------------------------------------------
    // Overlays
    // --------------------------------------------------------------------------

    fn snackbar(&mut self, ctx: &Context) {
        let Some(snack) = self.snackbar.clone() else {
            return;
        };
        if snack.at.elapsed() > Duration::from_secs(5) {
            self.snackbar = None;
            return;
        }
        let s = self.scheme.clone();
        let mut dismiss = false;
        egui::Area::new(Id::new("auroku-snackbar"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -54.0))
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(s.inverse_surface)
                    .corner_radius(10.0)
                    .inner_margin(Margin::symmetric(16, 10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(60),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                            let icon = match snack.tone {
                                Tone::Success => Icon::Check,
                                Tone::Error => Icon::Warning,
                                Tone::Warning => Icon::Warning,
                                _ => Icon::Info,
                            };
                            widgets::paint_icon(ui.painter(), rect, icon, s.inverse_on_surface);
                            ui.label(
                                RichText::new(&snack.text)
                                    .size(13.0)
                                    .color(s.inverse_on_surface),
                            );
                            let (close_rect, close_response) =
                                ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                            if close_response.hovered() {
                                ui.painter().circle_filled(
                                    close_rect.center(),
                                    15.0,
                                    Color32::from_white_alpha(28),
                                );
                            }
                            widgets::paint_icon(
                                ui.painter(),
                                Rect::from_center_size(close_rect.center(), vec2(15.0, 15.0)),
                                Icon::Close,
                                s.inverse_on_surface,
                            );
                            if close_response.clicked() {
                                dismiss = true;
                            }
                        });
                    });
            });
        if dismiss {
            self.snackbar = None;
        }
        ctx.request_repaint_after(Duration::from_millis(200));
    }

    fn confirm_modal(&mut self, ctx: &Context) {
        let Some(confirm) = self.confirm.clone() else {
            return;
        };
        let s = self.scheme.clone();
        let mut accept = false;
        let mut cancel = false;

        let response = egui::Modal::new(Id::new("auroku-confirm"))
            .frame(
                Frame::new()
                    .fill(s.surface_container_high)
                    .corner_radius(28.0)
                    .inner_margin(Margin::same(24)),
            )
            .backdrop_color(s.scrim)
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(
                    RichText::new(confirm.title())
                        .size(20.0)
                        .strong()
                        .color(s.on_surface),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(confirm.body())
                        .size(13.5)
                        .color(s.on_surface_variant),
                );
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(
                        ui,
                        &s,
                        None,
                        confirm.action_label(),
                        ButtonKind::Danger,
                        true,
                        0.0,
                    )
                    .clicked()
                    {
                        accept = true;
                    }
                    if button(ui, &s, None, "取消", ButtonKind::Text, true, 0.0).clicked() {
                        cancel = true;
                    }
                });
            });

        if response.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }

        if accept {
            self.confirm = None;
            self.run_confirm(confirm);
        } else if cancel {
            self.confirm = None;
        }
    }

    fn about_modal(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        let s = self.scheme.clone();
        let mut close = false;

        let response = egui::Modal::new(Id::new("auroku-about"))
            .frame(
                Frame::new()
                    .fill(s.surface_container_high)
                    .corner_radius(28.0)
                    .inner_margin(Margin::same(24)),
            )
            .backdrop_color(s.scrim)
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    let (logo, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                    ui.painter().rect_filled(logo, 14.0, s.primary_container);
                    widgets::paint_icon(
                        ui.painter(),
                        Rect::from_center_size(logo.center(), vec2(26.0, 26.0)),
                        Icon::Usb,
                        s.on_primary_container,
                    );
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("auRokku GUI")
                                .size(20.0)
                                .strong()
                                .color(s.on_surface),
                        );
                        ui.label(
                            RichText::new(i18n::tf(
                                "版本 {version}",
                                &[("version", env!("CARGO_PKG_VERSION"))],
                            ))
                            .size(12.0)
                            .color(s.on_surface_variant),
                        );
                    });
                });
                ui.add_space(14.0);
                ui.label(
                    RichText::new(i18n::t(
                        "基于 egui + eframe 的 Material 3 前端。协议、Type2Auth 算法与 JSON 结构均复用 Rust 后端，行为与 auRokku.py 保持一致。",
                    ))
                    .size(13.0)
                    .color(s.on_surface),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(i18n::t(
                        "跨平台：Linux（Wayland / X11）、Windows 与 macOS。USB 访问在 Linux 上通常需要 udev 规则或 root 权限。",
                    ))
                    .size(12.5)
                    .color(s.on_surface_variant),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(i18n::t(
                        "Linux 启动时会通过 pkexec（polkit）请求 root 权限；使用 --no-polkit 或 AUROKU_GUI_NO_POLKIT=1 可跳过。认证取消或 polkit 不可用时会以普通用户继续运行。",
                    ))
                    .size(12.0)
                    .color(s.on_surface_variant),
                );
                ui.add_space(18.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button(ui, &s, None, "关闭", ButtonKind::Filled, true, 0.0).clicked() {
                        close = true;
                    }
                });
            });

        if response.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if close {
            self.show_about = false;
        }
    }

    fn overlay_tick(&self, ctx: &Context) {
        if self.snackbar.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

impl eframe::App for AurokuApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if self.applied_dark != Some(self.dark) {
            self.scheme = if self.dark {
                M3Colors::dark()
            } else {
                M3Colors::light()
            };
            theme::apply(ctx, &self.scheme);
            self.applied_dark = Some(self.dark);
        }

        self.poll_events();
        if self.busy || self.scanning {
            self.worker.keep_alive();
        }

        let s = self.scheme.clone();

        egui::TopBottomPanel::top("auroku-appbar")
            .exact_height(72.0)
            .frame(
                Frame::new()
                    .fill(s.surface)
                    .inner_margin(Margin::symmetric(20, 10)),
            )
            .show(ctx, |ui| self.top_bar(ui));

        egui::TopBottomPanel::bottom("auroku-footer")
            .exact_height(30.0)
            .frame(
                Frame::new()
                    .fill(s.surface)
                    .inner_margin(Margin::symmetric(20, 4)),
            )
            .show(ctx, |ui| self.footer(ui));

        egui::SidePanel::left("auroku-rail")
            .exact_width(96.0)
            .resizable(false)
            .frame(
                Frame::new()
                    .fill(s.surface)
                    .inner_margin(Margin::symmetric(8, 12)),
            )
            .show(ctx, |ui| self.nav_rail(ui));

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(s.surface)
                    .inner_margin(Margin::symmetric(24, 16)),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        self.content(ui);
                        ui.add_space(24.0);
                    });
            });

        self.snackbar(ctx);
        self.confirm_modal(ctx);
        self.about_modal(ctx);
        self.overlay_tick(ctx);
    }
}

// ==============================================================================
// Value helpers
// ==============================================================================

fn value_str(value: &Value, key: &str) -> Option<String> {
    value.get(key).map(py_value)
}

fn py_value(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

fn key_value(ui: &mut Ui, scheme: &M3Colors, label: &str, value: &Value, key: &str) {
    if let Some(field) = value.get(key) {
        kv_row(ui, scheme, label, &py_value(field), true);
    }
}

fn parse_hex_u16(text: &str) -> Result<u16, String> {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u16::from_str_radix(digits, 16)
        .map_err(|_| i18n::tf("无效的十六进制值：{text}", &[("text", text)]))
}

fn parse_hex_u8(text: &str) -> Result<u8, String> {
    let trimmed = text.trim();
    let digits = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    u8::from_str_radix(digits, 16)
        .map_err(|_| i18n::tf("无效的十六进制字节：{text}", &[("text", text)]))
}
