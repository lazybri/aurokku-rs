#include "Theme.h"

#include <QApplication>

#include "Fonts.h"

namespace {

QColor blend(const QColor &from, const QColor &to, qreal amount)
{
    return QColor::fromRgbF(from.redF() + (to.redF() - from.redF()) * amount,
                            from.greenF() + (to.greenF() - from.greenF()) * amount,
                            from.blueF() + (to.blueF() - from.blueF()) * amount);
}

/// Fusion's standard light palette: the colours Qt applications use by default.
QPalette lightQtPalette()
{
    QPalette p;
    p.setColor(QPalette::Window, QColor(0xEF, 0xEF, 0xEF));
    p.setColor(QPalette::WindowText, QColor(0x1A, 0x1A, 0x1A));
    p.setColor(QPalette::Base, QColor(0xFF, 0xFF, 0xFF));
    p.setColor(QPalette::AlternateBase, QColor(0xF7, 0xF7, 0xF7));
    p.setColor(QPalette::ToolTipBase, QColor(0xFF, 0xFF, 0xDC));
    p.setColor(QPalette::ToolTipText, QColor(0x1A, 0x1A, 0x1A));
    p.setColor(QPalette::Text, QColor(0x1A, 0x1A, 0x1A));
    p.setColor(QPalette::PlaceholderText, QColor(0x8A, 0x8A, 0x8A));
    p.setColor(QPalette::Button, QColor(0xEF, 0xEF, 0xEF));
    p.setColor(QPalette::ButtonText, QColor(0x1A, 0x1A, 0x1A));
    p.setColor(QPalette::BrightText, QColor(0xD0, 0x00, 0x00));
    p.setColor(QPalette::Link, QColor(0x0A, 0x64, 0xA8));
    p.setColor(QPalette::Highlight, QColor(0x30, 0x8C, 0xC6));
    p.setColor(QPalette::HighlightedText, QColor(0xFF, 0xFF, 0xFF));
    p.setColor(QPalette::Light, QColor(0xFF, 0xFF, 0xFF));
    p.setColor(QPalette::Midlight, QColor(0xDC, 0xDC, 0xDC));
    p.setColor(QPalette::Mid, QColor(0xB8, 0xB8, 0xB8));
    p.setColor(QPalette::Dark, QColor(0x9F, 0x9F, 0x9F));
    p.setColor(QPalette::Shadow, QColor(0x76, 0x76, 0x76));
    p.setColor(QPalette::Disabled, QPalette::WindowText, QColor(0xB0, 0xB0, 0xB0));
    p.setColor(QPalette::Disabled, QPalette::Text, QColor(0xB0, 0xB0, 0xB0));
    p.setColor(QPalette::Disabled, QPalette::ButtonText, QColor(0xB0, 0xB0, 0xB0));
    p.setColor(QPalette::Disabled, QPalette::Highlight, QColor(0xE0, 0xE0, 0xE0));
    p.setColor(QPalette::Disabled, QPalette::HighlightedText, QColor(0xA0, 0xA0, 0xA0));
    return p;
}

QPalette darkQtPalette()
{
    QPalette p;
    p.setColor(QPalette::Window, QColor(0x2B, 0x2B, 0x2B));
    p.setColor(QPalette::WindowText, QColor(0xE6, 0xE6, 0xE6));
    p.setColor(QPalette::Base, QColor(0x1E, 0x1E, 0x1E));
    p.setColor(QPalette::AlternateBase, QColor(0x26, 0x26, 0x26));
    p.setColor(QPalette::ToolTipBase, QColor(0x2B, 0x2B, 0x2B));
    p.setColor(QPalette::ToolTipText, QColor(0xE6, 0xE6, 0xE6));
    p.setColor(QPalette::Text, QColor(0xE6, 0xE6, 0xE6));
    p.setColor(QPalette::PlaceholderText, QColor(0x8C, 0x8C, 0x8C));
    p.setColor(QPalette::Button, QColor(0x2B, 0x2B, 0x2B));
    p.setColor(QPalette::ButtonText, QColor(0xE6, 0xE6, 0xE6));
    p.setColor(QPalette::BrightText, QColor(0xFF, 0x6B, 0x6B));
    p.setColor(QPalette::Link, QColor(0x4C, 0xC2, 0xFF));
    p.setColor(QPalette::Highlight, QColor(0x2A, 0x82, 0xDA));
    p.setColor(QPalette::HighlightedText, QColor(0xFF, 0xFF, 0xFF));
    p.setColor(QPalette::Light, QColor(0x3B, 0x3B, 0x3B));
    p.setColor(QPalette::Midlight, QColor(0x33, 0x33, 0x33));
    p.setColor(QPalette::Mid, QColor(0x4A, 0x4A, 0x4A));
    p.setColor(QPalette::Dark, QColor(0x1C, 0x1C, 0x1C));
    p.setColor(QPalette::Shadow, QColor(0x12, 0x12, 0x12));
    p.setColor(QPalette::Disabled, QPalette::WindowText, QColor(0x7A, 0x7A, 0x7A));
    p.setColor(QPalette::Disabled, QPalette::Text, QColor(0x7A, 0x7A, 0x7A));
    p.setColor(QPalette::Disabled, QPalette::ButtonText, QColor(0x7A, 0x7A, 0x7A));
    p.setColor(QPalette::Disabled, QPalette::Highlight, QColor(0x3A, 0x3A, 0x3A));
    p.setColor(QPalette::Disabled, QPalette::HighlightedText, QColor(0x9A, 0x9A, 0x9A));
    return p;
}

Palette derivePalette(const QPalette &qt, bool dark)
{
    const QColor window = qt.color(QPalette::Window);
    const QColor base = qt.color(QPalette::Base);
    const QColor text = qt.color(QPalette::WindowText);
    const QColor highlight = qt.color(QPalette::Highlight);
    const QColor mid = qt.color(QPalette::Mid);
    const qreal tint = dark ? 0.38 : 0.18;

    Palette p;
    p.primary = highlight;
    p.onPrimary = qt.color(QPalette::HighlightedText);
    p.primaryContainer = blend(base, highlight, tint);
    p.onPrimaryContainer = text;

    p.secondary = blend(text, window, 0.25);
    p.onSecondary = base;
    p.secondaryContainer = blend(window, mid, 0.30);
    p.onSecondaryContainer = text;

    p.tertiary = dark ? QColor(0xC8, 0xA3, 0xE8) : QColor(0x6B, 0x3F, 0xA0);
    p.tertiaryContainer = blend(base, p.tertiary, tint);
    p.onTertiaryContainer = p.tertiary;

    p.surface = window;
    p.surfaceLowest = base;
    p.surfaceLow = qt.color(QPalette::AlternateBase);
    p.surfaceContainer = dark ? blend(window, Qt::white, 0.07) : base;
    p.surfaceHigh = dark ? blend(window, Qt::white, 0.13) : blend(window, mid, 0.30);
    p.surfaceHighest = dark ? blend(window, Qt::white, 0.17) : blend(window, mid, 0.50);
    p.onSurface = text;
    p.onSurfaceVariant = blend(text, window, 0.45);
    p.outline = dark ? blend(window, Qt::white, 0.22) : mid;
    p.outlineVariant = dark ? blend(window, Qt::white, 0.14) : qt.color(QPalette::Midlight);

    p.success = dark ? QColor(0x6C, 0xCB, 0x5F) : QColor(0x10, 0x7C, 0x10);
    p.successContainer = blend(base, p.success, tint);
    p.onSuccessContainer = p.success;
    p.warning = dark ? QColor(0xF0, 0xB4, 0x4C) : QColor(0x9A, 0x5B, 0x00);
    p.warningContainer = blend(base, p.warning, tint);
    p.onWarningContainer = p.warning;
    p.error = dark ? QColor(0xFF, 0x7B, 0x72) : QColor(0xC0, 0x2B, 0x1E);
    p.errorContainer = blend(base, p.error, tint);
    p.onErrorContainer = p.error;
    return p;
}

} // namespace

Theme &Theme::instance()
{
    static Theme instance;
    return instance;
}

Theme::Theme()
{
    rebuild();
}

void Theme::rebuild()
{
    m_qtPalette = m_dark ? darkQtPalette() : lightQtPalette();
    m_palette = derivePalette(m_qtPalette, m_dark);
}

void Theme::setDark(bool dark)
{
    if (m_dark == dark)
        return;
    m_dark = dark;
    rebuild();
    apply();
    emit changed();
}

void Theme::apply() const
{
    auto *app = qobject_cast<QApplication *>(QApplication::instance());
    if (app == nullptr)
        return;
    app->setPalette(m_qtPalette);
    app->setStyleSheet(styleSheet());
}

QString Theme::styleSheet() const
{
    const Palette p = m_palette;
    QString css = QStringLiteral(R"CSS(
QWidget#appRoot, QWidget#pageRoot, QMainWindow { background: @surface; }

QFrame#navRail { background: @surfaceLow; border-right: 1px solid @outlineVariant; }
QLabel#brandTitle { font-size: 15px; font-weight: 600; }
QLabel#brandSubtitle { font-size: 11px; color: @onSurfaceVariant; }

QPushButton#navButton { text-align: left; padding: 5px 8px; border: 1px solid transparent;
                        background: transparent; }
QPushButton#navButton:hover { background: @surfaceHigh; }
QPushButton#navButton:checked { background: @primaryContainer; color: @onPrimaryContainer; }
QPushButton#navButton:disabled { color: @onSurfaceVariant; }

QPushButton#segment { padding: 4px 12px; border: 1px solid transparent; background: transparent; }
QPushButton#segment:hover { background: @surfaceHigh; }
QPushButton#segment:checked { background: @primary; color: @onPrimary; }

QLabel#pageTitle { font-size: 19px; font-weight: 600; }
QLabel#pageSubtitle, QLabel#subtle, QLabel#kvLabel { color: @onSurfaceVariant; }
QLabel#subtle { font-size: 12px; }
QLabel#caption, QLabel#metricCaption { font-size: 11px; color: @onSurfaceVariant; }
QLabel#cardTitle { font-size: 13px; font-weight: 600; }
QLabel#metricValue { font-size: 14px; font-weight: 600; }
QLabel#metricValue[tone="primary"] { color: @primary; }
QLabel#metricValue[tone="success"] { color: @success; }
QLabel#metricValue[tone="warning"] { color: @warning; }
QLabel#metricValue[tone="error"] { color: @error; }
QLabel#metricValue[tone="tertiary"] { color: @tertiary; }
QLabel#metricValue[tone="neutral"] { color: @onSurface; }
QLabel#mono, QLabel#monoValue { font-family: "@mono"; }
QLabel#toastText { font-weight: 600; }

QFrame#card { background: @surfaceContainer; border: 1px solid @outlineVariant; }
QFrame#metric { background: @surfaceLow; border: 1px solid @outlineVariant; }

QFrame#banner { background: @surfaceHigh; border: 1px solid @outlineVariant; }
QFrame#banner[tone="primary"] { border-color: @primary; }
QFrame#banner[tone="success"] { border-color: @success; }
QFrame#banner[tone="warning"] { border-color: @warning; }
QFrame#banner[tone="error"] { border-color: @error; }
QFrame#banner[tone="tertiary"] { border-color: @tertiary; }
QLabel#bannerText { font-size: 12px; }
QLabel#bannerText[tone="primary"] { color: @primary; }
QLabel#bannerText[tone="success"] { color: @success; }
QLabel#bannerText[tone="warning"] { color: @warning; }
QLabel#bannerText[tone="error"] { color: @error; }
QLabel#bannerText[tone="tertiary"] { color: @tertiary; }

QLabel#chip { font-size: 11px; padding: 1px 6px; border: 1px solid @outline;
              color: @onSurfaceVariant; }
QLabel#chip[tone="primary"] { border-color: @primary; color: @primary; }
QLabel#chip[tone="success"] { border-color: @success; color: @success; }
QLabel#chip[tone="warning"] { border-color: @warning; color: @warning; }
QLabel#chip[tone="error"] { border-color: @error; color: @error; }
QLabel#chip[tone="tertiary"] { border-color: @tertiary; color: @tertiary; }

QFrame#footer { background: @surfaceLow; border-top: 1px solid @outlineVariant; }
QFrame#toast { background: @surfaceHighest; border: 1px solid @outline; }
QFrame#separator { background: @outlineVariant; border: none; min-height: 1px; max-height: 1px; }

QTextEdit#logView, QPlainTextEdit#codeBlock { font-family: "@mono"; }
)CSS");

    const auto replace = [&css](const QString &token, const QColor &color) {
        css.replace(token, color.name());
    };

    replace(QStringLiteral("@primaryContainer"), p.primaryContainer);
    replace(QStringLiteral("@onPrimaryContainer"), p.onPrimaryContainer);
    replace(QStringLiteral("@onPrimary"), p.onPrimary);
    replace(QStringLiteral("@primary"), p.primary);
    replace(QStringLiteral("@tertiary"), p.tertiary);
    replace(QStringLiteral("@success"), p.success);
    replace(QStringLiteral("@warning"), p.warning);
    replace(QStringLiteral("@error"), p.error);
    replace(QStringLiteral("@surfaceLowest"), p.surfaceLowest);
    replace(QStringLiteral("@surfaceContainer"), p.surfaceContainer);
    replace(QStringLiteral("@surfaceLow"), p.surfaceLow);
    replace(QStringLiteral("@surfaceHighest"), p.surfaceHighest);
    replace(QStringLiteral("@surfaceHigh"), p.surfaceHigh);
    replace(QStringLiteral("@surface"), p.surface);
    replace(QStringLiteral("@onSurfaceVariant"), p.onSurfaceVariant);
    replace(QStringLiteral("@onSurface"), p.onSurface);
    replace(QStringLiteral("@outlineVariant"), p.outlineVariant);
    replace(QStringLiteral("@outline"), p.outline);
    css.replace(QStringLiteral("@mono"), Fonts::monospaceFamily());

    return css;
}
