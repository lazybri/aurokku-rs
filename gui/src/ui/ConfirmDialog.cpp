#include "ConfirmDialog.h"

#include <QDialog>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

#include "I18n.h"
#include "Icons.h"
#include "Theme.h"
#include "Widgets.h"

namespace Dialogs {

bool confirm(QWidget *parent, const QString &titleKey, const QString &body,
             const QString &actionKey)
{
    QDialog dialog(parent);
    dialog.setWindowTitle(I18n::t(titleKey));
    dialog.setModal(true);
    dialog.setMinimumWidth(430);

    auto *layout = new QVBoxLayout(&dialog);
    layout->setContentsMargins(16, 14, 16, 12);
    layout->setSpacing(10);

    auto *header = new QHBoxLayout;
    header->setSpacing(12);
    auto *icon = new QLabel(&dialog);
    icon->setPixmap(Icons::pixmap(Icon::Warning, Theme::instance().palette().warning, 26));
    header->addWidget(icon, 0, Qt::AlignTop);
    auto *title = Ui::label(titleKey, QStringLiteral("cardTitle"), &dialog);
    title->setWordWrap(true);
    header->addWidget(title, 1);
    layout->addLayout(header);

    auto *text = new QLabel(body, &dialog);
    text->setWordWrap(true);
    text->setObjectName(QStringLiteral("subtle"));
    text->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(text);

    auto *buttons = new QHBoxLayout;
    buttons->addStretch(1);
    auto *cancel = Ui::button(QStringLiteral("取消"), Ui::ButtonVariant::Text, Icon::None, &dialog);
    auto *confirm = Ui::button(actionKey, Ui::ButtonVariant::Filled, Icon::Check, &dialog);
    buttons->addWidget(cancel);
    buttons->addWidget(confirm);
    layout->addLayout(buttons);

    QObject::connect(cancel, &QPushButton::clicked, &dialog, &QDialog::reject);
    QObject::connect(confirm, &QPushButton::clicked, &dialog, &QDialog::accept);

    return dialog.exec() == QDialog::Accepted;
}

} // namespace Dialogs
