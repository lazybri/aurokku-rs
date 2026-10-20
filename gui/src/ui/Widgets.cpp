#include "Widgets.h"

#include <QButtonGroup>
#include <QComboBox>
#include <QEvent>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QKeyEvent>
#include <QLayoutItem>
#include <QLineEdit>
#include <QMouseEvent>
#include <QPainter>
#include <QPalette>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QVariantAnimation>

#include "I18n.h"
#include "Theme.h"

namespace {

QColor colorForRole(const QString &role)
{
    const Palette &p = Theme::instance().palette();
    if (role == QLatin1String("primary"))
        return p.primary;
    if (role == QLatin1String("onPrimary"))
        return p.onPrimary;
    if (role == QLatin1String("onSecondaryContainer"))
        return p.onSecondaryContainer;
    if (role == QLatin1String("error"))
        return p.error;
    if (role == QLatin1String("onSurface"))
        return p.onSurface;
    return p.onSurfaceVariant;
}

QColor colorForTone(Tone tone)
{
    const Palette &p = Theme::instance().palette();
    switch (tone) {
    case Tone::Primary:
        return p.primary;
    case Tone::Success:
        return p.success;
    case Tone::Warning:
        return p.warning;
    case Tone::Error:
        return p.error;
    case Tone::Tertiary:
        return p.tertiary;
    case Tone::Neutral:
        break;
    }
    return p.onSurfaceVariant;
}

QString iconRoleForVariant(Ui::ButtonVariant variant)
{
    switch (variant) {
    case Ui::ButtonVariant::Filled:
        return QStringLiteral("onPrimary");
    case Ui::ButtonVariant::Tonal:
        return QStringLiteral("onSecondaryContainer");
    case Ui::ButtonVariant::DangerOutlined:
    case Ui::ButtonVariant::Danger:
        return QStringLiteral("error");
    case Ui::ButtonVariant::Outlined:
    case Ui::ButtonVariant::Text:
        return QStringLiteral("primary");
    }
    return QStringLiteral("primary");
}

Ui::ButtonVariant variantFromName(const QString &name)
{
    if (name == QLatin1String("filled"))
        return Ui::ButtonVariant::Filled;
    if (name == QLatin1String("tonal"))
        return Ui::ButtonVariant::Tonal;
    if (name == QLatin1String("outlined"))
        return Ui::ButtonVariant::Outlined;
    if (name == QLatin1String("text"))
        return Ui::ButtonVariant::Text;
    if (name == QLatin1String("danger"))
        return Ui::ButtonVariant::Danger;
    return Ui::ButtonVariant::DangerOutlined;
}

/// Keep the platform button shape; only differentiate text and dangerous actions.
void styleButton(QPushButton *button, Ui::ButtonVariant variant)
{
    button->setFlat(variant == Ui::ButtonVariant::Text);
    if (variant != Ui::ButtonVariant::Danger && variant != Ui::ButtonVariant::DangerOutlined)
        return;

    const QColor color = Theme::instance().palette().error;
    QPalette palette = button->palette();
    palette.setColor(QPalette::Active, QPalette::ButtonText, color);
    palette.setColor(QPalette::Inactive, QPalette::ButtonText, color);
    button->setPalette(palette);
}

void styleIconWidget(QWidget *widget, Icon icon, const QString &role, int size)
{
    widget->setProperty("iconId", static_cast<int>(icon));
    widget->setProperty("iconRole", role);
    widget->setProperty("iconSize", size);
}

} // namespace

namespace Ui {

const char *variantName(ButtonVariant variant)
{
    switch (variant) {
    case ButtonVariant::Filled:
        return "filled";
    case ButtonVariant::Tonal:
        return "tonal";
    case ButtonVariant::Outlined:
        return "outlined";
    case ButtonVariant::Text:
        return "text";
    case ButtonVariant::DangerOutlined:
        return "dangerOutlined";
    case ButtonVariant::Danger:
        return "danger";
    }
    return "filled";
}

const char *toneName(Tone tone)
{
    switch (tone) {
    case Tone::Primary:
        return "primary";
    case Tone::Success:
        return "success";
    case Tone::Warning:
        return "warning";
    case Tone::Error:
        return "error";
    case Tone::Tertiary:
        return "tertiary";
    case Tone::Neutral:
        break;
    }
    return "neutral";
}

QLabel *label(const QString &sourceKey, const QString &objectName, QWidget *parent)
{
    auto *widget = new QLabel(I18n::t(sourceKey), parent);
    widget->setProperty("i18nKey", sourceKey);
    if (!objectName.isEmpty())
        widget->setObjectName(objectName);
    return widget;
}

QLabel *valueLabel(const QString &text, bool mono, QWidget *parent)
{
    auto *widget = new QLabel(text, parent);
    if (mono)
        widget->setObjectName(QStringLiteral("monoValue"));
    widget->setTextInteractionFlags(Qt::TextSelectableByMouse);
    return widget;
}

QPushButton *button(const QString &sourceKey, ButtonVariant variant, Icon icon, QWidget *parent)
{
    auto *widget = new QPushButton(I18n::t(sourceKey), parent);
    widget->setProperty("i18nKey", sourceKey);
    widget->setProperty("variant", QString::fromLatin1(variantName(variant)));
    widget->setCursor(Qt::PointingHandCursor);
    widget->setIconSize(QSize(18, 18));
    styleButton(widget, variant);
    if (icon != Icon::None) {
        widget->setIcon(Icons::icon(icon, colorForRole(iconRoleForVariant(variant)), 18));
        styleIconWidget(widget, icon, iconRoleForVariant(variant), 18);
    }
    return widget;
}

QPushButton *dynamicButton(const QString &text, ButtonVariant variant, Icon icon, QWidget *parent)
{
    auto *widget = new QPushButton(text, parent);
    widget->setProperty("variant", QString::fromLatin1(variantName(variant)));
    widget->setCursor(Qt::PointingHandCursor);
    widget->setIconSize(QSize(18, 18));
    styleButton(widget, variant);
    if (icon != Icon::None) {
        widget->setIcon(Icons::icon(icon, colorForRole(iconRoleForVariant(variant)), 18));
        styleIconWidget(widget, icon, iconRoleForVariant(variant), 18);
    }
    return widget;
}

void usbActionHint(QWidget *control)
{
    if (control == nullptr)
        return;

    static const QString key = QStringLiteral("该操作会通过 USB 与设备通信，设备可能重启");
    control->setProperty("i18nTip", key);
    control->setToolTip(I18n::t(key));
}

QFrame *card(QWidget *parent)
{
    auto *frame = new QFrame(parent);
    frame->setObjectName(QStringLiteral("card"));
    frame->setFrameShape(QFrame::NoFrame);
    auto *layout = new QVBoxLayout(frame);
    layout->setContentsMargins(16, 14, 16, 14);
    layout->setSpacing(8);
    return frame;
}

QVBoxLayout *cardBody(QFrame *frame)
{
    return qobject_cast<QVBoxLayout *>(frame->layout());
}

QWidget *cardHeader(Icon icon, const QString &titleKey, const QString &subtitleKey, QWidget *parent)
{
    auto *header = new QWidget(parent);
    auto *layout = new QHBoxLayout(header);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(12);

    auto *iconLabel = new QLabel(header);
    iconLabel->setFixedSize(24, 24);
    iconLabel->setPixmap(Icons::pixmap(icon, Theme::instance().palette().primary, 22));
    styleIconWidget(iconLabel, icon, QStringLiteral("primary"), 22);
    layout->addWidget(iconLabel, 0, Qt::AlignTop);

    auto *text = new QVBoxLayout;
    text->setContentsMargins(0, 0, 0, 0);
    text->setSpacing(1);
    text->addWidget(label(titleKey, QStringLiteral("cardTitle"), header));
    if (!subtitleKey.isEmpty())
        text->addWidget(label(subtitleKey, QStringLiteral("subtle"), header));
    layout->addLayout(text, 1);
    return header;
}

QFrame *banner(Icon icon, const QString &sourceKey, Tone tone, QWidget *parent)
{
    auto *frame = new QFrame(parent);
    frame->setObjectName(QStringLiteral("banner"));
    frame->setProperty("tone", QString::fromLatin1(toneName(tone)));
    frame->setFrameShape(QFrame::NoFrame);

    auto *layout = new QHBoxLayout(frame);
    layout->setContentsMargins(10, 8, 10, 8);
    layout->setSpacing(8);

    auto *iconLabel = new QLabel(frame);
    iconLabel->setFixedSize(20, 20);
    styleIconWidget(iconLabel, icon,
                    QStringLiteral("tone:%1").arg(QString::fromLatin1(toneName(tone))), 18);
    iconLabel->setPixmap(Icons::pixmap(icon, colorForTone(tone), 18));

    layout->addWidget(iconLabel, 0, Qt::AlignTop);

    auto *text = new QLabel(frame);
    text->setObjectName(QStringLiteral("bannerText"));
    text->setProperty("tone", QString::fromLatin1(toneName(tone)));
    text->setProperty("i18nKey", sourceKey);
    text->setText(I18n::t(sourceKey));
    text->setWordWrap(true);
    text->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(text, 1);
    return frame;
}

QFrame *bannerText(Icon icon, const QString &text, Tone tone, QWidget *parent)
{
    QFrame *frame = banner(icon, QString(), tone, parent);
    auto *body = frame->findChild<QLabel *>(QStringLiteral("bannerText"));
    if (body != nullptr) {
        body->setProperty("i18nKey", QVariant());
        body->setText(text);
    }
    return frame;
}

QLabel *chip(const QString &text, Tone tone, QWidget *parent)
{
    auto *widget = new QLabel(text, parent);
    widget->setObjectName(QStringLiteral("chip"));
    widget->setProperty("tone", QString::fromLatin1(toneName(tone)));
    widget->setAlignment(Qt::AlignCenter);
    widget->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Fixed);
    widget->setTextInteractionFlags(Qt::TextSelectableByMouse);
    return widget;
}

QWidget *metric(Icon icon, const QString &captionKey, const QString &value, Tone tone, QWidget *parent)
{
    QWidget *widget = metricText(icon, I18n::t(captionKey), value, tone, parent);
    if (auto *caption = widget->findChild<QLabel *>(QStringLiteral("metricCaption")))
        caption->setProperty("i18nKey", captionKey);
    return widget;
}

QWidget *metricText(Icon icon, const QString &caption, const QString &value, Tone tone, QWidget *parent)
{
    auto *frame = new QFrame(parent);
    frame->setObjectName(QStringLiteral("metric"));
    auto *layout = new QVBoxLayout(frame);
    layout->setContentsMargins(12, 10, 12, 10);
    layout->setSpacing(2);

    auto *top = new QHBoxLayout;
    top->setContentsMargins(0, 0, 0, 0);
    top->setSpacing(8);

    auto *iconLabel = new QLabel(frame);
    iconLabel->setFixedSize(18, 18);
    const QColor color = tone == Tone::Neutral ? Theme::instance().palette().onSurfaceVariant
                                               : colorForTone(tone);
    styleIconWidget(iconLabel, icon,
                    QStringLiteral("tone:%1").arg(QString::fromLatin1(toneName(tone))), 17);
    iconLabel->setPixmap(Icons::pixmap(icon, color, 17));
    top->addWidget(iconLabel);

    auto *captionLabel = new QLabel(caption, frame);
    captionLabel->setObjectName(QStringLiteral("metricCaption"));
    top->addWidget(captionLabel, 1);
    layout->addLayout(top);

    auto *valueLabel = new QLabel(value, frame);
    valueLabel->setObjectName(QStringLiteral("metricValue"));
    valueLabel->setProperty("tone", QString::fromLatin1(toneName(tone)));
    valueLabel->setWordWrap(true);
    valueLabel->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(valueLabel);
    return frame;
}

QWidget *kvRow(const QString &labelKey, const QString &value, bool mono, QWidget *parent)
{
    QWidget *row = kvRowText(I18n::t(labelKey), value, mono, parent);
    if (auto *labelWidget = row->findChild<QLabel *>(QStringLiteral("kvLabel")))
        labelWidget->setProperty("i18nKey", labelKey);
    return row;
}

QWidget *kvRowText(const QString &label, const QString &value, bool mono, QWidget *parent)
{
    auto *row = new QWidget(parent);
    auto *layout = new QHBoxLayout(row);
    layout->setContentsMargins(0, 2, 0, 2);
    layout->setSpacing(12);

    auto *key = new QLabel(label, row);
    key->setObjectName(QStringLiteral("kvLabel"));
    key->setMinimumWidth(150);
    key->setAlignment(Qt::AlignLeft | Qt::AlignTop);
    key->setWordWrap(true);
    layout->addWidget(key, 0);

    auto *valueLabel = new QLabel(value, row);
    if (mono)
        valueLabel->setObjectName(QStringLiteral("monoValue"));
    valueLabel->setWordWrap(true);
    valueLabel->setTextInteractionFlags(Qt::TextSelectableByMouse | Qt::TextSelectableByKeyboard);
    layout->addWidget(valueLabel, 1);
    return row;
}

QWidget *emptyState(Icon icon, const QString &titleKey, const QString &subtitleKey, QWidget *parent)
{
    auto *widget = new QWidget(parent);
    auto *layout = new QVBoxLayout(widget);
    layout->setContentsMargins(16, 26, 16, 26);
    layout->setSpacing(6);
    layout->setAlignment(Qt::AlignCenter);

    auto *iconLabel = new QLabel(widget);
    iconLabel->setPixmap(Icons::pixmap(icon, Theme::instance().palette().outline, 36));
    iconLabel->setAlignment(Qt::AlignCenter);
    styleIconWidget(iconLabel, icon, QStringLiteral("outline"), 36);
    layout->addWidget(iconLabel, 0, Qt::AlignHCenter);

    auto *title = label(titleKey, QStringLiteral("cardTitle"), widget);
    title->setAlignment(Qt::AlignCenter);
    layout->addWidget(title);

    auto *subtitle = label(subtitleKey, QStringLiteral("subtle"), widget);
    subtitle->setAlignment(Qt::AlignCenter);
    subtitle->setWordWrap(true);
    layout->addWidget(subtitle);
    return widget;
}

QPlainTextEdit *codeBlock(const QString &text, int height, QWidget *parent)
{
    auto *edit = new QPlainTextEdit(parent);
    edit->setObjectName(QStringLiteral("codeBlock"));
    edit->setReadOnly(true);
    edit->setPlainText(text);
    edit->setLineWrapMode(QPlainTextEdit::NoWrap);
    edit->setMinimumHeight(height);
    edit->setMaximumHeight(height);
    return edit;
}

QFrame *separator(QWidget *parent)
{
    auto *line = new QFrame(parent);
    line->setObjectName(QStringLiteral("separator"));
    line->setFrameShape(QFrame::HLine);
    line->setFixedHeight(1);
    return line;
}

QVBoxLayout *vbox(QWidget *parent, int margin, int spacing)
{
    auto *layout = new QVBoxLayout(parent);
    layout->setContentsMargins(margin, margin, margin, margin);
    layout->setSpacing(spacing);
    return layout;
}

QHBoxLayout *hbox(QWidget *parent, int margin, int spacing)
{
    auto *layout = new QHBoxLayout(parent);
    layout->setContentsMargins(margin, margin, margin, margin);
    layout->setSpacing(spacing);
    return layout;
}

void retranslateTree(QWidget *root)
{
    if (root == nullptr)
        return;

    const QList<QWidget *> widgets = root->findChildren<QWidget *>();
    for (QWidget *widget : widgets) {
        const QVariant key = widget->property("i18nKey");
        if (key.isValid() && !key.toString().isEmpty()) {
            const QString translated = I18n::t(key.toString());
            if (auto *label = qobject_cast<QLabel *>(widget))
                label->setText(translated);
            else if (auto *button = qobject_cast<QAbstractButton *>(widget))
                button->setText(translated);
            else if (auto *group = qobject_cast<QGroupBox *>(widget))
                group->setTitle(translated);
            else
                widget->setWindowTitle(translated);
        }
        const QVariant tip = widget->property("i18nTip");
        if (tip.isValid() && !tip.toString().isEmpty())
            widget->setToolTip(I18n::t(tip.toString()));

        const QVariant placeholder = widget->property("i18nPlaceholder");
        if (placeholder.isValid() && !placeholder.toString().isEmpty()) {
            const QString text = I18n::t(placeholder.toString());
            if (auto *edit = qobject_cast<QLineEdit *>(widget))
                edit->setPlaceholderText(text);
            else if (auto *combo = qobject_cast<QComboBox *>(widget))
                combo->setPlaceholderText(text);
            else if (auto *textEdit = qobject_cast<QTextEdit *>(widget))
                textEdit->setPlaceholderText(text);
            else if (auto *plain = qobject_cast<QPlainTextEdit *>(widget))
                plain->setPlaceholderText(text);
        }
    }

    refreshIcons(root);
}

void refreshIcons(QWidget *root)
{
    if (root == nullptr)
        return;

    const Palette &palette = Theme::instance().palette();
    const auto resolveRole = [&palette](const QString &role) -> QColor {
        if (role.startsWith(QLatin1String("tone:"))) {
            const QString tone = role.mid(5);
            if (tone == QLatin1String("primary"))
                return palette.primary;
            if (tone == QLatin1String("success"))
                return palette.success;
            if (tone == QLatin1String("warning"))
                return palette.warning;
            if (tone == QLatin1String("error"))
                return palette.error;
            if (tone == QLatin1String("tertiary"))
                return palette.tertiary;
            return palette.onSurfaceVariant;
        }
        if (role == QLatin1String("primary"))
            return palette.primary;
        if (role == QLatin1String("onPrimary"))
            return palette.onPrimary;
        if (role == QLatin1String("onSecondaryContainer"))
            return palette.onSecondaryContainer;
        if (role == QLatin1String("error"))
            return palette.error;
        if (role == QLatin1String("errorContainer"))
            return palette.onErrorContainer;
        if (role == QLatin1String("onSurface"))
            return palette.onSurface;
        if (role == QLatin1String("outline"))
            return palette.outline;
        return palette.onSurfaceVariant;
    };

    const QList<QWidget *> widgets = root->findChildren<QWidget *>();
    for (QWidget *widget : widgets) {
        if (auto *button = qobject_cast<QPushButton *>(widget)) {
            const QVariant variant = button->property("variant");
            if (variant.isValid())
                styleButton(button, variantFromName(variant.toString()));
        }
        if (!widget->property("iconId").isValid())
            continue;
        const auto icon = static_cast<Icon>(widget->property("iconId").toInt());
        const QColor color = resolveRole(widget->property("iconRole").toString());
        const int size = widget->property("iconSize").toInt();

        if (auto *label = qobject_cast<QLabel *>(widget))
            label->setPixmap(Icons::pixmap(icon, color, size > 0 ? size : 20));
        else if (auto *button = qobject_cast<QPushButton *>(widget))
            button->setIcon(Icons::icon(icon, color, size > 0 ? size : 18));
    }
}

void clearLayout(QLayout *layout)
{
    if (layout == nullptr)
        return;
    while (QLayoutItem *item = layout->takeAt(0)) {
        if (QWidget *widget = item->widget())
            widget->deleteLater();
        else if (QLayout *child = item->layout())
            clearLayout(child);
        delete item;
    }
}

} // namespace Ui

// ==============================================================================
// ToggleSwitch
// ==============================================================================

ToggleSwitch::ToggleSwitch(bool checked, QWidget *parent)
    : QWidget(parent)
    , m_checked(checked)
    , m_position(checked ? 1.0 : 0.0)
{
    setCursor(Qt::PointingHandCursor);
    setFocusPolicy(Qt::StrongFocus);
    setFixedSize(46, 26);
    setAccessibleName(QStringLiteral("toggle"));
}

void ToggleSwitch::setChecked(bool checked, bool animate)
{
    if (m_checked == checked)
        return;
    m_checked = checked;

    auto *animation = new QVariantAnimation(this);
    animation->setDuration(animate ? 140 : 0);
    animation->setStartValue(m_position);
    animation->setEndValue(checked ? 1.0 : 0.0);
    animation->setEasingCurve(QEasingCurve::OutCubic);
    connect(animation, &QVariantAnimation::valueChanged, this, [this](const QVariant &value) {
        m_position = value.toReal();
        update();
    });
    connect(animation, &QVariantAnimation::finished, animation, &QObject::deleteLater);
    animation->start();

    emit toggled(checked);
    update();
}

QSize ToggleSwitch::sizeHint() const
{
    return QSize(46, 26);
}

void ToggleSwitch::paintEvent(QPaintEvent *)
{
    const Palette &p = Theme::instance().palette();
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing, true);

    const qreal knobRadius = 8.5;
    const QRectF track(1.0, 4.0, width() - 2.0, height() - 8.0);
    const qreal centerX = track.left() + knobRadius + 1.5 + m_position * (track.width() - 2 * knobRadius - 3.0);
    const QPointF knobCenter(centerX, track.center().y());

    QColor trackColor = m_checked ? p.primary : (m_hovered ? p.outline : p.outlineVariant);
    if (!isEnabled())
        trackColor = p.surfaceHigh;
    painter.setPen(Qt::NoPen);
    painter.setBrush(trackColor);
    painter.drawRoundedRect(track, track.height() / 2.0, track.height() / 2.0);

    QColor knobColor = m_checked ? p.onPrimary
                                 : (Theme::instance().dark() ? p.onSurface : p.surfaceLowest);
    if (!isEnabled())
        knobColor = p.outlineVariant;
    painter.setBrush(knobColor);
    painter.drawEllipse(knobCenter, knobRadius, knobRadius);

    if (hasFocus()) {
        const QPen focusPen(p.primary, 1.0, Qt::DashLine);
        painter.setPen(focusPen);
        painter.setBrush(Qt::NoBrush);
        painter.drawRect(rect().adjusted(0, 0, -1, -1));
    }
}

void ToggleSwitch::mouseReleaseEvent(QMouseEvent *event)
{
    if (event->button() == Qt::LeftButton && rect().contains(event->position().toPoint()))
        setChecked(!m_checked);
    QWidget::mouseReleaseEvent(event);
}

void ToggleSwitch::keyPressEvent(QKeyEvent *event)
{
    if (event->key() == Qt::Key_Space || event->key() == Qt::Key_Return
        || event->key() == Qt::Key_Enter) {
        setChecked(!m_checked);
        event->accept();
        return;
    }
    QWidget::keyPressEvent(event);
}

void ToggleSwitch::enterEvent(QEnterEvent *event)
{
    m_hovered = true;
    update();
    QWidget::enterEvent(event);
}

void ToggleSwitch::leaveEvent(QEvent *event)
{
    m_hovered = false;
    update();
    QWidget::leaveEvent(event);
}

// ==============================================================================
// SegmentedControl
// ==============================================================================

SegmentedControl::SegmentedControl(const QStringList &sourceKeys, QWidget *parent)
    : QFrame(parent)
{
    setObjectName(QStringLiteral("segmented"));
    m_layout = new QHBoxLayout(this);
    m_layout->setContentsMargins(0, 0, 0, 0);
    m_layout->setSpacing(0);
    m_group = new QButtonGroup(this);
    m_group->setExclusive(true);

    connect(m_group, &QButtonGroup::idClicked, this, [this](int id) {
        if (id == m_index)
            return;
        m_index = id;
        emit currentChanged(id);
    });

    setKeys(sourceKeys);
}

QString SegmentedControl::currentKey() const
{
    return m_keys.value(m_index);
}

void SegmentedControl::setCurrentIndex(int index)
{
    if (index < 0 || index >= m_buttons.size() || index == m_index)
        return;
    m_index = index;
    m_buttons[index]->setChecked(true);
}

void SegmentedControl::setKeys(const QStringList &sourceKeys)
{
    for (QPushButton *button : m_buttons) {
        m_group->removeButton(button);
        m_layout->removeWidget(button);
        button->deleteLater();
    }
    m_buttons.clear();
    m_keys.clear();

    for (int i = 0; i < sourceKeys.size(); ++i) {
        const QString &key = sourceKeys.at(i);
        auto *button = new QPushButton(I18n::t(key), this);
        button->setObjectName(QStringLiteral("segment"));
        button->setProperty("i18nKey", key);
        button->setCheckable(true);
        button->setCursor(Qt::PointingHandCursor);
        button->setChecked(i == m_index);
        m_group->addButton(button, i);
        m_layout->addWidget(button);
        m_buttons.append(button);
        m_keys.append(key);
    }
    if (!m_buttons.isEmpty())
        m_buttons[qBound(0, m_index, m_buttons.size() - 1)]->setChecked(true);
}
