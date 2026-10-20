#pragma once

#include <QString>

class QWidget;

namespace Dialogs {
/// Modal confirmation with a translation-aware title/body/action label.
bool confirm(QWidget *parent, const QString &titleKey, const QString &body,
             const QString &actionKey);
} // namespace Dialogs
