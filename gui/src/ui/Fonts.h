#pragma once

#include <QString>

/// Font discovery and application-wide font setup.
///
/// The UI is designed around a CJK-capable sans font; on systems without one
/// the caller may fall back to English (matching the previous front-end).
class Fonts {
public:
    /// Resolve the best available UI font family.
    static QString uiFamily();
    /// Resolve a monospace family with CJK fallback.
    static QString monospaceFamily();
    /// True when at least one CJK-capable family is available.
    static bool hasCjk();
    /// Install `AUROKU_GUI_FONT` (family name or file path) and the app font.
    static void apply();
};
