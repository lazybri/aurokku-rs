#pragma once

#include <QColor>
#include <QIcon>

/// Small hand-painted icon set (24x24 design grid) so the UI does not depend
/// on external resources or platform theme icons.
enum class Icon {
    None,
    Dashboard,
    Lock,
    Key,
    Probe,
    List,
    Power,
    Terminal,
    Usb,
    Scan,
    Write,
    Trash,
    Refresh,
    Play,
    Copy,
    Info,
    Warning,
    Stop,
    Sun,
    Moon,
    About,
    Check,
    Clear,
    Globe,
    ChevronDown
};

namespace Icons {
QIcon icon(Icon id, const QColor &color, int size = 20);
QPixmap pixmap(Icon id, const QColor &color, int size);
} // namespace Icons
