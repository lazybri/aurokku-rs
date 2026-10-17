//! System CJK font discovery.
//!
//! egui ships Latin-only default fonts. The GUI is localised for Chinese, so we
//! load a system CJK font at start-up when one is available. A user override can
//! be supplied with `AUROKU_GUI_FONT=/path/to/font.ttf`.
//!
//! This keeps the binary small (no bundled font) and works on Linux (Wayland or
//! X11), Windows and macOS.

use std::sync::Arc;

use eframe::egui;

const FONT_ENV: &str = "AUROKU_GUI_FONT";

#[cfg(target_os = "linux")]
const CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJKsc-Regular.otf",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    "/usr/share/fonts/truetype/arphic/uming.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

#[cfg(target_os = "windows")]
const CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyh.ttf",
    r"C:\Windows\Fonts\msyhl.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
    r"C:\Windows\Fonts\segoeui.ttf",
];

#[cfg(target_os = "macos")]
const CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
];

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
const CANDIDATES: &[&str] = &[];

/// Install default fonts plus the first available CJK fallback.
///
/// Returns `true` when a CJK font was found. The caller uses this to fall back
/// to the English UI on systems without any CJK font.
pub fn install(ctx: &egui::Context) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let mut cjk_loaded = false;

    if let Some(bytes) = load_font() {
        let name = "system-cjk".to_owned();
        fonts
            .font_data
            .insert(name.clone(), Arc::new(egui::FontData::from_owned(bytes)));

        // Append as a fallback so Latin text keeps egui's default glyph metrics.
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
        cjk_loaded = true;
    }

    ctx.set_fonts(fonts);
    cjk_loaded
}

fn load_font() -> Option<Vec<u8>> {
    let mut paths: Vec<String> = Vec::new();
    if let Ok(path) = std::env::var(FONT_ENV) {
        if !path.trim().is_empty() {
            paths.push(path);
        }
    }
    paths.extend(CANDIDATES.iter().map(|path| (*path).to_owned()));

    for path in paths {
        match std::fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                eprintln!("[auroku-gui] loaded CJK font {path}");
                return Some(bytes);
            }
            _ => {}
        }
    }
    eprintln!("[auroku-gui] no CJK font found; Chinese labels may render as boxes");
    None
}
