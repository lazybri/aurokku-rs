//! Material Design 3 colour schemes and egui style integration.

use eframe::egui::{
    self, vec2, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style,
    TextStyle, Visuals,
};

/// Material 3 colour roles used by the UI.
///
/// Light values are derived from the teal seed `#006A6A`; dark values are the
/// matching tonal counterparts. Names follow the M3 specification.
#[derive(Clone, Debug)]
#[allow(dead_code)] // complete M3 role palette, not every role is used yet
pub struct M3Colors {
    pub dark: bool,
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary: Color32,
    pub on_secondary: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub tertiary: Color32,
    pub tertiary_container: Color32,
    pub on_tertiary_container: Color32,
    pub surface: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub error: Color32,
    pub on_error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub inverse_surface: Color32,
    pub inverse_on_surface: Color32,
    pub success: Color32,
    pub success_container: Color32,
    pub on_success_container: Color32,
    pub warning: Color32,
    pub warning_container: Color32,
    pub on_warning_container: Color32,
    pub scrim: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

impl M3Colors {
    /// Light scheme (default).
    pub fn light() -> Self {
        Self {
            dark: false,
            primary: rgb(0x006A6A),
            on_primary: rgb(0xFFFFFF),
            primary_container: rgb(0x9CF1F0),
            on_primary_container: rgb(0x002020),
            secondary: rgb(0x4A6363),
            on_secondary: rgb(0xFFFFFF),
            secondary_container: rgb(0xCCE8E7),
            on_secondary_container: rgb(0x051F1F),
            tertiary: rgb(0x4B607C),
            tertiary_container: rgb(0xD3E4FF),
            on_tertiary_container: rgb(0x041C35),
            surface: rgb(0xF7FAF9),
            surface_container_lowest: rgb(0xFFFFFF),
            surface_container_low: rgb(0xF1F4F3),
            surface_container: rgb(0xEBEEED),
            surface_container_high: rgb(0xE5E9E8),
            surface_container_highest: rgb(0xDFE3E2),
            on_surface: rgb(0x191C1C),
            on_surface_variant: rgb(0x3F4949),
            outline: rgb(0x6F7979),
            outline_variant: rgb(0xBEC9C8),
            error: rgb(0xBA1A1A),
            on_error: rgb(0xFFFFFF),
            error_container: rgb(0xFFDAD6),
            on_error_container: rgb(0x410002),
            inverse_surface: rgb(0x2D3131),
            inverse_on_surface: rgb(0xEFF1F0),
            success: rgb(0x2E6B30),
            success_container: rgb(0xB7F0B2),
            on_success_container: rgb(0x002106),
            warning: rgb(0x7A5900),
            warning_container: rgb(0xFFDEA6),
            on_warning_container: rgb(0x271900),
            scrim: Color32::from_black_alpha(120),
        }
    }

    /// Dark scheme.
    pub fn dark() -> Self {
        Self {
            dark: true,
            primary: rgb(0x80D5D4),
            on_primary: rgb(0x003737),
            primary_container: rgb(0x004F4F),
            on_primary_container: rgb(0x9CF1F0),
            secondary: rgb(0xB0CCCB),
            on_secondary: rgb(0x1B3534),
            secondary_container: rgb(0x324B4B),
            on_secondary_container: rgb(0xCCE8E7),
            tertiary: rgb(0xB3C8E8),
            tertiary_container: rgb(0x334863),
            on_tertiary_container: rgb(0xD3E4FF),
            surface: rgb(0x0E1514),
            surface_container_lowest: rgb(0x090F0F),
            surface_container_low: rgb(0x161D1D),
            surface_container: rgb(0x1A2121),
            surface_container_high: rgb(0x242B2B),
            surface_container_highest: rgb(0x2F3636),
            on_surface: rgb(0xE0E3E2),
            on_surface_variant: rgb(0xBEC9C8),
            outline: rgb(0x889392),
            outline_variant: rgb(0x3F4949),
            error: rgb(0xFFB4AB),
            on_error: rgb(0x690005),
            error_container: rgb(0x93000A),
            on_error_container: rgb(0xFFDAD6),
            inverse_surface: rgb(0xE0E3E2),
            inverse_on_surface: rgb(0x2D3131),
            warning: rgb(0xF5BD48),
            warning_container: rgb(0x5C4200),
            on_warning_container: rgb(0xFFDEA6),
            success: rgb(0x9DD89A),
            success_container: rgb(0x11521A),
            on_success_container: rgb(0xB7F0B2),
            scrim: Color32::from_black_alpha(160),
        }
    }
}

/// Build the egui visuals for a Material 3 colour scheme.
pub fn visuals(scheme: &M3Colors) -> Visuals {
    let mut visuals = if scheme.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.override_text_color = Some(scheme.on_surface);
    visuals.panel_fill = scheme.surface;
    visuals.window_fill = scheme.surface_container_low;
    visuals.window_stroke = Stroke::new(1.0_f32, scheme.outline_variant);
    visuals.window_corner_radius = CornerRadius::same(28);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if scheme.dark { 90 } else { 36 }),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if scheme.dark { 80 } else { 28 }),
    };
    visuals.faint_bg_color = scheme.surface_container_low;
    visuals.extreme_bg_color = scheme.surface_container_high;
    visuals.text_edit_bg_color = Some(scheme.surface_container_lowest);
    visuals.code_bg_color = scheme.surface_container_high;
    visuals.hyperlink_color = scheme.primary;
    visuals.warn_fg_color = scheme.warning;
    visuals.error_fg_color = scheme.error;
    visuals.selection = egui::style::Selection {
        bg_fill: alpha(scheme.primary, if scheme.dark { 70 } else { 46 }),
        stroke: Stroke::new(1.0_f32, scheme.primary),
    };
    visuals.striped = true;
    visuals.button_frame = true;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = scheme.surface_container_low;
    widgets.noninteractive.weak_bg_fill = scheme.surface_container_low;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, scheme.outline_variant);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, scheme.on_surface);
    widgets.noninteractive.corner_radius = CornerRadius::same(12);

    widgets.inactive.bg_fill = scheme.surface_container_high;
    widgets.inactive.weak_bg_fill = scheme.surface_container_high;
    widgets.inactive.bg_stroke = Stroke::NONE;
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, scheme.on_surface);
    widgets.inactive.corner_radius = CornerRadius::same(12);

    widgets.hovered.bg_fill = scheme.surface_container_highest;
    widgets.hovered.weak_bg_fill = scheme.surface_container_highest;
    widgets.hovered.bg_stroke = Stroke::NONE;
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, scheme.on_surface);
    widgets.hovered.corner_radius = CornerRadius::same(12);

    widgets.active.bg_fill = scheme.secondary_container;
    widgets.active.weak_bg_fill = scheme.secondary_container;
    widgets.active.bg_stroke = Stroke::NONE;
    widgets.active.fg_stroke = Stroke::new(1.0_f32, scheme.on_secondary_container);
    widgets.active.corner_radius = CornerRadius::same(12);

    widgets.open.bg_fill = scheme.surface_container_high;
    widgets.open.weak_bg_fill = scheme.surface_container_high;
    widgets.open.bg_stroke = Stroke::NONE;
    widgets.open.fg_stroke = Stroke::new(1.0_f32, scheme.on_surface);
    widgets.open.corner_radius = CornerRadius::same(12);

    visuals
}

/// Apply the Material 3 scheme to an egui context.
pub fn apply(ctx: &egui::Context, scheme: &M3Colors) {
    let mut style: Style = (*ctx.style()).clone();
    style.visuals = visuals(scheme);
    style.animation_time = 0.16;
    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(18.0, 9.0);
    style.spacing.interact_size = vec2(40.0, 40.0);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.window_margin = Margin::same(24);
    style.spacing.combo_width = 240.0;

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(26.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
}

/// Convert a colour to a translucent variant (alpha as 0..=255).
pub fn alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

/// Blend `amount` (0..=1) of `over` into `base`.
pub fn mix(base: Color32, over: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    Color32::from_rgb(
        blend(base.r(), over.r()),
        blend(base.g(), over.g()),
        blend(base.b(), over.b()),
    )
}
