#pragma once

#include <QColor>
#include <QObject>
#include <QPalette>
#include <QString>

/// Semantic colours derived from the active Qt palette.
struct Palette {
    QColor primary;
    QColor onPrimary;
    QColor primaryContainer;
    QColor onPrimaryContainer;
    QColor secondary;
    QColor onSecondary;
    QColor secondaryContainer;
    QColor onSecondaryContainer;
    QColor tertiary;
    QColor tertiaryContainer;
    QColor onTertiaryContainer;

    QColor surface;
    QColor surfaceLowest;
    QColor surfaceLow;
    QColor surfaceContainer;
    QColor surfaceHigh;
    QColor surfaceHighest;
    QColor onSurface;
    QColor onSurfaceVariant;
    QColor outline;
    QColor outlineVariant;

    QColor success;
    QColor successContainer;
    QColor onSuccessContainer;
    QColor warning;
    QColor warningContainer;
    QColor onWarningContainer;
    QColor error;
    QColor errorContainer;
    QColor onErrorContainer;
};

/// Global light/dark theme: a plain Qt palette plus a small style sheet for the
/// few custom container widgets.
class Theme : public QObject {
    Q_OBJECT
public:
    static Theme &instance();

    bool dark() const { return m_dark; }
    void setDark(bool dark);

    const Palette &palette() const { return m_palette; }
    const QPalette &qtPalette() const { return m_qtPalette; }

    /// Apply the current palette and style sheet to the whole application.
    void apply() const;

    QString styleSheet() const;

signals:
    void changed();

private:
    Theme();
    void rebuild();

    bool m_dark = true;
    Palette m_palette;
    QPalette m_qtPalette;
};
