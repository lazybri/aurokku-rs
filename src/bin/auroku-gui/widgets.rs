//! Small Material 3 component toolkit built on egui primitives.
//!
//! egui has no Material widget set, so the pieces here paint the M3 shapes and
//! state layers directly (filled / tonal / outlined buttons, switches, chips,
//! cards, segmented controls, vector icons). Everything is immediate-mode and
//! therefore works identically on Wayland, X11, Windows and macOS.

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, FontId, Frame, Layout, Margin, Pos2, Rect, Response,
    RichText, Sense, Stroke, StrokeKind, TextStyle, Ui,
};

use crate::i18n;
use crate::theme::{alpha, mix, M3Colors};

// ==============================================================================
// Icons (painted as vectors so they never depend on an emoji/system font)
// ==============================================================================

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // a few icons are reserved for future screens
pub enum Icon {
    Dashboard,
    Lock,
    Key,
    Probe,
    List,
    Power,
    Terminal,
    Refresh,
    Scan,
    Copy,
    Contrast,
    Info,
    Check,
    Close,
    ChevronDown,
    Usb,
    Write,
    Trash,
    Play,
    Stop,
    ArrowRight,
    Warning,
}

/// Paint a vector icon inside `rect` using `color`.
pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let w = rect.width();
    let stroke = Stroke::new((w * 0.085).clamp(1.2, 2.1), color);
    let p = |x: f32, y: f32| {
        pos2(
            rect.left() + x * rect.width(),
            rect.top() + y * rect.height(),
        )
    };
    let center = rect.center();
    let arc = |cx: f32, cy: f32, r: f32, start: f32, end: f32, segments: usize| {
        let points: Vec<Pos2> = (0..=segments)
            .map(|i| {
                let t = i as f32 / segments as f32;
                let a = start + (end - start) * t;
                pos2(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));
    };

    match icon {
        Icon::Dashboard => {
            let gap = w * 0.1;
            let cell = (w - gap) * 0.5;
            for (dx, dy) in [
                (0.0, 0.0),
                (cell + gap, 0.0),
                (0.0, cell + gap),
                (cell + gap, cell + gap),
            ] {
                let square =
                    Rect::from_min_size(pos2(rect.left() + dx, rect.top() + dy), vec2(cell, cell));
                painter.rect_filled(square, w * 0.09, color);
            }
        }
        Icon::Lock => {
            let body = Rect::from_min_max(p(0.12, 0.44), p(0.88, 0.92));
            let shackle_center = p(0.5, 0.46);
            let radius = w * 0.27;
            arc(
                shackle_center.x,
                shackle_center.y,
                radius,
                std::f32::consts::PI,
                std::f32::consts::TAU,
                18,
            );
            painter.rect_filled(body, w * 0.11, color);
        }
        Icon::Key => {
            painter.circle_stroke(p(0.25, 0.5), w * 0.17, stroke);
            painter.line_segment([p(0.4, 0.5), p(0.9, 0.5)], stroke);
            painter.line_segment([p(0.72, 0.5), p(0.72, 0.7)], stroke);
            painter.line_segment([p(0.84, 0.5), p(0.84, 0.64)], stroke);
        }
        Icon::Probe => {
            painter.add(egui::Shape::line(
                vec![
                    p(0.04, 0.56),
                    p(0.28, 0.56),
                    p(0.39, 0.24),
                    p(0.55, 0.8),
                    p(0.66, 0.44),
                    p(0.96, 0.44),
                ],
                stroke,
            ));
        }
        Icon::List => {
            for i in 0..3 {
                let y = 0.22 + 0.28 * i as f32;
                painter.circle_filled(p(0.1, y), w * 0.055, color);
                painter.line_segment([p(0.26, y), p(0.92, y)], stroke);
            }
        }
        Icon::Power => {
            let c = p(0.5, 0.56);
            arc(
                c.x,
                c.y,
                w * 0.31,
                -std::f32::consts::FRAC_PI_2 + 0.62,
                std::f32::consts::TAU - std::f32::consts::FRAC_PI_2 - 0.62,
                20,
            );
            painter.line_segment([p(0.5, 0.08), p(0.5, 0.5)], stroke);
        }
        Icon::Terminal => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.05, 0.13), p(0.95, 0.87)),
                w * 0.12,
                stroke,
                StrokeKind::Inside,
            );
            painter.add(egui::Shape::line(
                vec![p(0.18, 0.36), p(0.34, 0.5), p(0.18, 0.64)],
                stroke,
            ));
            painter.line_segment([p(0.46, 0.68), p(0.7, 0.68)], stroke);
        }
        Icon::Refresh => {
            let c = p(0.5, 0.5);
            arc(
                c.x,
                c.y,
                w * 0.32,
                -std::f32::consts::PI * 0.2,
                std::f32::consts::PI * 1.35,
                20,
            );
            let tip = p(0.82, 0.2);
            painter.add(egui::Shape::line(
                vec![p(0.66, 0.18), tip, p(0.88, 0.34)],
                stroke,
            ));
        }
        Icon::Scan => {
            painter.circle_stroke(p(0.42, 0.42), w * 0.27, stroke);
            painter.line_segment([p(0.63, 0.63), p(0.9, 0.9)], stroke);
        }
        Icon::Copy => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.08, 0.08), p(0.68, 0.68)),
                w * 0.1,
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.32, 0.32), p(0.92, 0.92)),
                w * 0.1,
                stroke,
                StrokeKind::Inside,
            );
        }
        Icon::Contrast => {
            let r = w * 0.38;
            painter.circle_stroke(center, r, stroke);
            let half: Vec<Pos2> = (0..=18)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 18.0;
                    pos2(center.x + r * a.cos(), center.y + r * a.sin())
                })
                .collect();
            painter.add(egui::Shape::convex_polygon(half, color, Stroke::NONE));
        }
        Icon::Info => {
            painter.circle_stroke(center, w * 0.4, stroke);
            painter.circle_filled(p(0.5, 0.28), w * 0.065, color);
            painter.line_segment([p(0.5, 0.44), p(0.5, 0.74)], stroke);
        }
        Icon::Check => {
            painter.add(egui::Shape::line(
                vec![p(0.12, 0.54), p(0.4, 0.8), p(0.9, 0.22)],
                stroke,
            ));
        }
        Icon::Close => {
            painter.line_segment([p(0.2, 0.2), p(0.8, 0.8)], stroke);
            painter.line_segment([p(0.8, 0.2), p(0.2, 0.8)], stroke);
        }
        Icon::ChevronDown => {
            painter.add(egui::Shape::line(
                vec![p(0.25, 0.4), p(0.5, 0.65), p(0.75, 0.4)],
                stroke,
            ));
        }
        Icon::Usb => {
            painter.line_segment([p(0.5, 0.18), p(0.5, 0.82)], stroke);
            painter.circle_filled(p(0.5, 0.88), w * 0.09, color);
            painter.add(egui::Shape::line(
                vec![p(0.32, 0.28), p(0.5, 0.16), p(0.68, 0.28)],
                stroke,
            ));
            painter.add(egui::Shape::line(
                vec![p(0.5, 0.42), p(0.28, 0.55), p(0.28, 0.66)],
                stroke,
            ));
            painter.circle_filled(p(0.28, 0.7), w * 0.055, color);
            painter.add(egui::Shape::line(vec![p(0.5, 0.58), p(0.72, 0.7)], stroke));
            painter.rect_filled(
                Rect::from_center_size(p(0.72, 0.74), vec2(w * 0.14, w * 0.14)),
                w * 0.02,
                color,
            );
        }
        Icon::Write => {
            painter.line_segment([p(0.18, 0.82), p(0.82, 0.18)], stroke);
            painter.line_segment([p(0.18, 0.82), p(0.1, 0.9)], stroke);
            painter.line_segment([p(0.62, 0.18), p(0.82, 0.38)], stroke);
        }
        Icon::Trash => {
            painter.line_segment([p(0.2, 0.28), p(0.8, 0.28)], stroke);
            painter.rect_stroke(
                Rect::from_min_max(p(0.26, 0.28), p(0.74, 0.9)),
                w * 0.08,
                stroke,
                StrokeKind::Inside,
            );
            painter.line_segment([p(0.4, 0.18), p(0.6, 0.18)], stroke);
            painter.line_segment([p(0.42, 0.44), p(0.42, 0.76)], stroke);
            painter.line_segment([p(0.58, 0.44), p(0.58, 0.76)], stroke);
        }
        Icon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.3, 0.16), p(0.86, 0.5), p(0.3, 0.84)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            painter.rect_filled(
                Rect::from_min_max(p(0.24, 0.24), p(0.76, 0.76)),
                w * 0.08,
                color,
            );
        }
        Icon::ArrowRight => {
            painter.line_segment([p(0.12, 0.5), p(0.8, 0.5)], stroke);
            painter.add(egui::Shape::line(
                vec![p(0.6, 0.28), p(0.84, 0.5), p(0.6, 0.72)],
                stroke,
            ));
        }
        Icon::Warning => {
            painter.add(egui::Shape::line(
                vec![p(0.5, 0.12), p(0.94, 0.86), p(0.06, 0.86), p(0.5, 0.12)],
                stroke,
            ));
            painter.line_segment([p(0.5, 0.4), p(0.5, 0.64)], stroke);
            painter.circle_filled(p(0.5, 0.75), w * 0.05, color);
        }
    }
}

// ==============================================================================
// Buttons
// ==============================================================================

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Filled,
    Tonal,
    Outlined,
    Text,
    Danger,
    DangerOutlined,
}

fn button_colors(kind: ButtonKind, scheme: &M3Colors) -> (Color32, Color32, Stroke) {
    match kind {
        ButtonKind::Filled => (scheme.primary, scheme.on_primary, Stroke::NONE),
        ButtonKind::Tonal => (
            scheme.secondary_container,
            scheme.on_secondary_container,
            Stroke::NONE,
        ),
        ButtonKind::Outlined => (
            Color32::TRANSPARENT,
            scheme.primary,
            Stroke::new(1.0_f32, scheme.outline),
        ),
        ButtonKind::Text => (Color32::TRANSPARENT, scheme.primary, Stroke::NONE),
        ButtonKind::Danger => (scheme.error, scheme.on_error, Stroke::NONE),
        ButtonKind::DangerOutlined => (
            Color32::TRANSPARENT,
            scheme.error,
            Stroke::new(1.0_f32, scheme.outline_variant),
        ),
    }
}

fn button_hover_fill(kind: ButtonKind, scheme: &M3Colors, base: Color32) -> Color32 {
    match kind {
        ButtonKind::Filled | ButtonKind::Danger => mix(base, Color32::WHITE, 0.08),
        ButtonKind::Tonal => mix(base, scheme.on_secondary_container, 0.08),
        ButtonKind::Outlined | ButtonKind::Text | ButtonKind::DangerOutlined => {
            let fg = if kind == ButtonKind::DangerOutlined {
                scheme.error
            } else {
                scheme.primary
            };
            alpha(fg, if scheme.dark { 40 } else { 22 })
        }
    }
}

/// Material 3 button with an optional leading icon.
pub fn button(
    ui: &mut Ui,
    scheme: &M3Colors,
    icon: Option<Icon>,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    min_width: f32,
) -> Response {
    let label = i18n::t(label);
    let font = FontId::proportional(14.0);
    let text_width = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    });
    let icon_space = if icon.is_some() { 26.0 } else { 0.0 };
    let width = (text_width + icon_space + 40.0).max(min_width);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 40.0), sense);

    let (base, fg, stroke) = button_colors(kind, scheme);
    let (fg, stroke) = if enabled {
        (fg, stroke)
    } else {
        (
            scheme.on_surface_variant,
            Stroke::new(1.0_f32, scheme.outline_variant),
        )
    };
    let fill = if !enabled {
        if kind == ButtonKind::Filled || kind == ButtonKind::Danger || kind == ButtonKind::Tonal {
            alpha(base, 60)
        } else {
            Color32::TRANSPARENT
        }
    } else if response.hovered() {
        button_hover_fill(kind, scheme, base)
    } else {
        base
    };

    let painter = ui.painter();
    painter.rect_filled(rect, 20.0, fill);
    if stroke != Stroke::NONE {
        painter.rect_stroke(rect.shrink(0.5), 20.0, stroke, StrokeKind::Inside);
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            22.0,
            Stroke::new(2.0_f32, alpha(scheme.primary, 120)),
            StrokeKind::Outside,
        );
    }

    let total = text_width + icon_space;
    let mut x = rect.center().x - total * 0.5;
    if let Some(icon) = icon {
        paint_icon(
            painter,
            Rect::from_center_size(pos2(x + 9.0, rect.center().y), vec2(18.0, 18.0)),
            icon,
            fg,
        );
        x += icon_space;
    }
    painter.text(
        pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        fg,
    );

    if enabled {
        response
    } else {
        response.on_hover_text(i18n::t("当前操作不可用"))
    }
}

/// Circular icon button (M3 icon button, standard 40dp).
pub fn icon_button(
    ui: &mut Ui,
    scheme: &M3Colors,
    icon: Icon,
    tooltip: &str,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), sense);
    let painter = ui.painter();
    if response.hovered() && enabled {
        painter.circle_filled(rect.center(), 20.0, scheme.surface_container_high);
    }
    if response.has_focus() {
        painter.circle_stroke(
            rect.center(),
            20.0,
            Stroke::new(2.0_f32, alpha(scheme.primary, 120)),
        );
    }
    paint_icon(
        painter,
        Rect::from_center_size(rect.center(), vec2(20.0, 20.0)),
        icon,
        if enabled {
            scheme.on_surface_variant
        } else {
            alpha(scheme.on_surface_variant, 80)
        },
    );
    response.on_hover_text(i18n::t(tooltip))
}

// ==============================================================================
// Switch / segmented control
// ==============================================================================

/// Material 3 switch. Clicking the track or label toggles the value.
pub fn switch(ui: &mut Ui, scheme: &M3Colors, value: &mut bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::click());
    if response.clicked() {
        *value = !*value;
    }
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("m3-switch"), *value, 0.14);
    let track = if *value {
        scheme.primary
    } else {
        scheme.surface_container_highest
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 16.0, track);
    if !*value {
        painter.rect_stroke(
            rect.shrink(0.5),
            16.0,
            Stroke::new(1.0_f32, scheme.outline),
            StrokeKind::Inside,
        );
    }
    let x = rect.left() + 16.0 + (rect.width() - 32.0) * t;
    let radius = 8.0 + 4.0 * t;
    let thumb = if *value {
        scheme.on_primary
    } else {
        scheme.outline
    };
    painter.circle_filled(pos2(x, rect.center().y), radius, thumb);
    response
}

/// A row of selectable pill buttons behaving like an M3 segmented control.
pub fn segmented<T: Clone + PartialEq>(
    ui: &mut Ui,
    scheme: &M3Colors,
    options: &[(T, &str)],
    selected: &mut T,
) -> bool {
    let mut changed = false;
    Frame::new()
        .fill(scheme.surface_container_low)
        .stroke(Stroke::new(1.0_f32, scheme.outline_variant))
        .corner_radius(22.0)
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (value, label) in options {
                    let label = i18n::t(label);
                    let is_selected = value == selected;
                    let font = FontId::proportional(13.5);
                    let text_width = ui.fonts_mut(|fonts| {
                        fonts
                            .layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE)
                            .size()
                            .x
                    });
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(text_width + 30.0, 34.0), Sense::click());
                    if response.clicked() {
                        *selected = value.clone();
                        changed = true;
                    }
                    let fill = if is_selected {
                        scheme.secondary_container
                    } else if response.hovered() {
                        scheme.surface_container_high
                    } else {
                        Color32::TRANSPARENT
                    };
                    let fg = if is_selected {
                        scheme.on_secondary_container
                    } else {
                        scheme.on_surface_variant
                    };
                    let painter = ui.painter();
                    painter.rect_filled(rect, 17.0, fill);
                    painter.text(rect.center(), Align2::CENTER_CENTER, label, font, fg);
                }
            });
        });
    changed
}

// ==============================================================================
// Text field
// ==============================================================================

/// Outlined Material 3 text field with a small label above.
#[allow(clippy::too_many_arguments)]
pub fn text_field(
    ui: &mut Ui,
    scheme: &M3Colors,
    label: &str,
    hint: &str,
    value: &mut String,
    width: f32,
    monospace: bool,
    enabled: bool,
) -> Response {
    let label = i18n::t(label);
    let hint = i18n::t(hint);
    ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(11.5)
                .color(scheme.on_surface_variant),
        );
        ui.add_space(1.0);
        let frame = Frame::new()
            .fill(scheme.surface_container_lowest)
            .stroke(Stroke::new(1.0_f32, scheme.outline))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width((width - 26.0).max(40.0));
                let mut edit = egui::TextEdit::singleline(value)
                    .frame(false)
                    .hint_text(RichText::new(hint).color(scheme.on_surface_variant))
                    .desired_width(f32::INFINITY)
                    .text_color(scheme.on_surface);
                if monospace {
                    edit = edit.font(TextStyle::Monospace);
                }
                ui.add_enabled(enabled, edit)
            });
        if frame.inner.has_focus() {
            ui.painter().rect_stroke(
                frame.response.rect,
                10.0,
                Stroke::new(2.0_f32, scheme.primary),
                StrokeKind::Inside,
            );
        }
        frame.inner
    })
    .inner
}

// ==============================================================================
// Surfaces
// ==============================================================================

/// Outlined M3 card.
pub fn card<R>(ui: &mut Ui, scheme: &M3Colors, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(scheme.surface_container_low)
        .stroke(Stroke::new(1.0_f32, scheme.outline_variant))
        .corner_radius(20.0)
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Card title row with a small tonal icon.
pub fn card_header(ui: &mut Ui, scheme: &M3Colors, icon: Icon, title: &str, subtitle: &str) {
    let title = i18n::t(title);
    let subtitle = i18n::t(subtitle);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, 12.0, scheme.secondary_container);
        paint_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(19.0, 19.0)),
            icon,
            scheme.on_secondary_container,
        );
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(title)
                    .size(16.0)
                    .strong()
                    .color(scheme.on_surface),
            );
            if !subtitle.is_empty() {
                ui.label(
                    RichText::new(subtitle)
                        .size(11.5)
                        .color(scheme.on_surface_variant),
                );
            }
        });
    });
    ui.add_space(4.0);
}

/// Page heading.
pub fn page_header(ui: &mut Ui, scheme: &M3Colors, title: &str, subtitle: &str) {
    let title = i18n::t(title);
    let subtitle = i18n::t(subtitle);
    ui.label(
        RichText::new(title)
            .size(30.0)
            .strong()
            .color(scheme.on_surface),
    );
    ui.label(
        RichText::new(subtitle)
            .size(13.5)
            .color(scheme.on_surface_variant),
    );
    ui.add_space(8.0);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Primary,
    Tertiary,
    Success,
    Error,
    Warning,
}

fn tone_colors(tone: Tone, scheme: &M3Colors) -> (Color32, Color32) {
    match tone {
        Tone::Neutral => (scheme.surface_container_high, scheme.on_surface_variant),
        Tone::Primary => (scheme.primary_container, scheme.on_primary_container),
        Tone::Tertiary => (scheme.tertiary_container, scheme.on_tertiary_container),
        Tone::Success => (scheme.success_container, scheme.on_success_container),
        Tone::Error => (scheme.error_container, scheme.on_error_container),
        Tone::Warning => (scheme.warning_container, scheme.on_warning_container),
    }
}

/// Small status chip with a leading dot.
pub fn chip(ui: &mut Ui, scheme: &M3Colors, label: &str, tone: Tone) -> Response {
    let label = i18n::t(label);
    let font = FontId::proportional(12.0);
    let text_width = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    });
    let (rect, response) = ui.allocate_exact_size(vec2(text_width + 30.0, 26.0), Sense::hover());
    let (bg, fg) = tone_colors(tone, scheme);
    let painter = ui.painter();
    painter.rect_filled(rect, 13.0, bg);
    painter.circle_filled(pos2(rect.left() + 12.0, rect.center().y), 3.5, fg);
    painter.text(
        pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        fg,
    );
    response
}

/// Inline informational banner.
pub fn banner(ui: &mut Ui, scheme: &M3Colors, icon: Icon, text: &str, tone: Tone) {
    let text = i18n::t(text);
    let (bg, fg) = tone_colors(tone, scheme);
    Frame::new()
        .fill(bg)
        .corner_radius(14.0)
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                paint_icon(ui.painter(), rect, icon, fg);
                ui.add_space(2.0);
                ui.label(RichText::new(text).size(12.5).color(fg));
            });
        });
}

/// Metric tile used on the overview / status pages.
pub fn metric(ui: &mut Ui, scheme: &M3Colors, icon: Icon, label: &str, value: &str, tone: Tone) {
    let label = i18n::t(label);
    let value = i18n::t(value);
    let (bg, fg) = tone_colors(tone, scheme);
    Frame::new()
        .fill(scheme.surface_container_lowest)
        .stroke(Stroke::new(1.0_f32, scheme.outline_variant))
        .corner_radius(16.0)
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
                ui.painter().rect_filled(rect, 12.0, bg);
                paint_icon(
                    ui.painter(),
                    Rect::from_center_size(rect.center(), vec2(19.0, 19.0)),
                    icon,
                    fg,
                );
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(label)
                            .size(11.0)
                            .color(scheme.on_surface_variant),
                    );
                    ui.label(
                        RichText::new(value)
                            .size(15.0)
                            .strong()
                            .color(scheme.on_surface),
                    );
                });
            });
        });
}

/// Label/value row for detail lists.
pub fn kv_row(ui: &mut Ui, scheme: &M3Colors, key: &str, value: &str, mono: bool) {
    let key = i18n::t(key);
    let value = i18n::t(value);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(key)
                .size(12.5)
                .color(scheme.on_surface_variant),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let text = if mono {
                RichText::new(value)
                    .monospace()
                    .size(12.5)
                    .color(scheme.on_surface)
            } else {
                RichText::new(value).size(12.5).color(scheme.on_surface)
            };
            ui.label(text);
        });
    });
}

/// Read-only monospaced block (JSON, raw hex, logs).
pub fn code_block(ui: &mut Ui, scheme: &M3Colors, text: &str, max_height: f32) {
    Frame::new()
        .fill(scheme.surface_container_high)
        .corner_radius(14.0)
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(text)
                                .monospace()
                                .size(12.0)
                                .color(scheme.on_surface),
                        )
                        .wrap(),
                    );
                });
        });
}

/// Empty-state placeholder.
pub fn empty_state(ui: &mut Ui, scheme: &M3Colors, icon: Icon, title: &str, body: &str) {
    let title = i18n::t(title);
    let body = i18n::t(body);
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, 18.0, scheme.surface_container_high);
        paint_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(28.0, 28.0)),
            icon,
            scheme.on_surface_variant,
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new(title)
                .size(14.5)
                .strong()
                .color(scheme.on_surface),
        );
        ui.label(
            RichText::new(body)
                .size(12.0)
                .color(scheme.on_surface_variant),
        );
        ui.add_space(12.0);
    });
}

/// Small helper for a left-aligned section label.
pub fn subtle_label(ui: &mut Ui, scheme: &M3Colors, text: &str) {
    let text = i18n::t(text);
    ui.label(
        RichText::new(text)
            .size(11.5)
            .color(scheme.on_surface_variant),
    );
}
