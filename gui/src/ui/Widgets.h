#pragma once

#include <QFrame>
#include <QIcon>
#include <QLabel>
#include <QString>
#include <QStringList>
#include <QWidget>

#include "core/AppState.h"
#include "ui/Icons.h"

class QBoxLayout;
class QButtonGroup;
class QHBoxLayout;
class QPlainTextEdit;
class QPushButton;
class QVBoxLayout;

/// Qt-leaning widget factories shared by every page.
namespace Ui {

enum class ButtonVariant { Filled, Tonal, Outlined, Text, DangerOutlined, Danger };

const char *variantName(ButtonVariant variant);
const char *toneName(Tone tone);

/// Label whose text follows the interface language.
QLabel *label(const QString &sourceKey, const QString &objectName = {}, QWidget *parent = nullptr);
/// Dynamic (already translated) text.
QLabel *valueLabel(const QString &text, bool mono = true, QWidget *parent = nullptr);

QPushButton *button(const QString &sourceKey, ButtonVariant variant, Icon icon = Icon::None,
                    QWidget *parent = nullptr);
/// Button with dynamic text (results, counters).
QPushButton *dynamicButton(const QString &text, ButtonVariant variant, Icon icon = Icon::None,
                           QWidget *parent = nullptr);

/// Attach the shared "device may restart" hint to a control that talks to the
/// device over USB.
void usbActionHint(QWidget *control);

/// Card container with a ready-to-use vertical body layout.
QFrame *card(QWidget *parent = nullptr);
QVBoxLayout *cardBody(QFrame *frame);
QWidget *cardHeader(Icon icon, const QString &titleKey, const QString &subtitleKey,
                    QWidget *parent = nullptr);

/// Toned notice. `sourceKey` is translated automatically.
QFrame *banner(Icon icon, const QString &sourceKey, Tone tone, QWidget *parent = nullptr);
/// Notice with dynamic (already translated) text.
QFrame *bannerText(Icon icon, const QString &text, Tone tone, QWidget *parent = nullptr);

QLabel *chip(const QString &text, Tone tone, QWidget *parent = nullptr);

QWidget *metric(Icon icon, const QString &captionKey, const QString &value, Tone tone,
                QWidget *parent = nullptr);
QWidget *metricText(Icon icon, const QString &caption, const QString &value, Tone tone,
                    QWidget *parent = nullptr);

QWidget *kvRow(const QString &labelKey, const QString &value, bool mono = true,
               QWidget *parent = nullptr);
QWidget *kvRowText(const QString &label, const QString &value, bool mono = true,
                   QWidget *parent = nullptr);

QWidget *emptyState(Icon icon, const QString &titleKey, const QString &subtitleKey,
                    QWidget *parent = nullptr);

QPlainTextEdit *codeBlock(const QString &text, int height, QWidget *parent = nullptr);

QFrame *separator(QWidget *parent = nullptr);
QVBoxLayout *vbox(QWidget *parent, int margin = 0, int spacing = 8);
QHBoxLayout *hbox(QWidget *parent, int margin = 0, int spacing = 8);

/// Re-apply translations (`i18nKey`) and repaint theme-dependent icons.
void retranslateTree(QWidget *root);
void refreshIcons(QWidget *root);
/// Delete every item (and widget) from a layout.
void clearLayout(QLayout *layout);

} // namespace Ui

/// Animated on/off switch.
class ToggleSwitch : public QWidget {
    Q_OBJECT
public:
    explicit ToggleSwitch(bool checked = false, QWidget *parent = nullptr);

    bool isChecked() const { return m_checked; }
    void setChecked(bool checked, bool animate = true);

    QSize sizeHint() const override;

signals:
    void toggled(bool checked);

protected:
    void paintEvent(QPaintEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;
    void keyPressEvent(QKeyEvent *event) override;
    void enterEvent(QEnterEvent *event) override;
    void leaveEvent(QEvent *event) override;

private:
    bool m_checked = false;
    qreal m_position = 0.0;
    bool m_hovered = false;
};

/// Segmented button group.
class SegmentedControl : public QFrame {
    Q_OBJECT
public:
    explicit SegmentedControl(const QStringList &sourceKeys, QWidget *parent = nullptr);

    int currentIndex() const { return m_index; }
    QString currentKey() const;
    void setCurrentIndex(int index);
    void setKeys(const QStringList &sourceKeys);

signals:
    void currentChanged(int index);

private:
    QVector<QString> m_keys;
    QVector<QPushButton *> m_buttons;
    QHBoxLayout *m_layout = nullptr;
    QButtonGroup *m_group = nullptr;
    int m_index = 0;
};
