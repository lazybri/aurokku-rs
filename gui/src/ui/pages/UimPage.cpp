#include "ui/pages/UimPage.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QSignalBlocker>
#include <QVBoxLayout>

#include "core/JsonHelpers.h"
#include "ui/I18n.h"
#include "ui/Icons.h"

UimPage::UimPage(PageHost *host, QWidget *parent)
    : Page(host, parent)
{
    addPageHeader(QStringLiteral("UIM 锁"),
                  QStringLiteral("读取、写入或清除 SIM 锁绑定（0x54 / 0x72 / 0x57）"));
    m_content->addWidget(Ui::banner(
        Icon::Warning,
        QStringLiteral("写入或清除锁会修改 SIM 绑定状态。执行前请确认设备与卡片匹配，避免写入错误绑定值。"),
        Tone::Warning));

    buildReadCard();
    buildWriteCard();
    buildClearCard();
    buildMaintenanceCard();
    addLastResultCard();
}

void UimPage::buildReadCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Lock, QStringLiteral("读取锁状态"),
                                   QStringLiteral("进入 UIMLK 会话并读取 control 位与绑定值"), frame));

    auto *button = track(
        Ui::button(QStringLiteral("读取 UIM 锁"), Ui::ButtonVariant::Filled, Icon::Scan, frame));
    Ui::usbActionHint(button);
    body->addWidget(button, 0, Qt::AlignLeft);
    connect(button, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("readuimlk")}},
                          QStringLiteral("读取 UIM 锁"));
    });

    m_readResultHost = new QWidget(frame);
    m_readResultLayout = new QVBoxLayout(m_readResultHost);
    m_readResultLayout->setContentsMargins(0, 0, 0, 0);
    m_readResultLayout->setSpacing(6);
    body->addWidget(m_readResultHost);

    m_content->addWidget(frame);
}

void UimPage::buildWriteCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Write, QStringLiteral("写入绑定值"),
                                   QStringLiteral("0x72：写 4 字节锁值并校验回读"), frame));

    auto *row = new QHBoxLayout;
    row->setSpacing(16);

    auto *valueBox = new QWidget(frame);
    auto *valueLayout = new QVBoxLayout(valueBox);
    valueLayout->setContentsMargins(0, 0, 0, 0);
    valueLayout->setSpacing(3);
    valueLayout->addWidget(Ui::label(QStringLiteral("锁值（8 位 hex）"),
                                     QStringLiteral("caption"), valueBox));
    m_writeValue = new QLineEdit(state().writelkValue, valueBox);
    m_writeValue->setMinimumWidth(220);
    valueLayout->addWidget(m_writeValue);
    row->addWidget(valueBox);

    auto *controlBox = new QWidget(frame);
    auto *controlLayout = new QVBoxLayout(controlBox);
    controlLayout->setContentsMargins(0, 0, 0, 0);
    controlLayout->setSpacing(3);
    controlLayout->addWidget(Ui::label(QStringLiteral("控制位"), QStringLiteral("caption"),
                                       controlBox));
    auto *controlRow = new QHBoxLayout;
    controlRow->setSpacing(8);
    m_writeControl = new ToggleSwitch(state().writelkControl, controlBox);
    m_writeControlValue = new QLabel(controlBox);
    m_writeControlValue->setObjectName(QStringLiteral("monoValue"));
    m_writeControlValue->setText(state().writelkControl ? QStringLiteral("1") : QStringLiteral("0"));
    controlRow->addWidget(m_writeControl);
    controlRow->addWidget(m_writeControlValue);
    controlRow->addStretch(1);
    controlLayout->addLayout(controlRow);
    row->addWidget(controlBox);
    row->addStretch(1);
    body->addLayout(row);

    auto *write = track(
        Ui::button(QStringLiteral("写入绑定值"), Ui::ButtonVariant::Tonal, Icon::Write, frame));
    Ui::usbActionHint(write);
    body->addWidget(write, 0, Qt::AlignLeft);

    connect(m_writeValue, &QLineEdit::textChanged, this,
            [this](const QString &text) { state().writelkValue = text; });
    connect(m_writeControl, &ToggleSwitch::toggled, this, [this](bool checked) {
        state().writelkControl = checked;
        m_writeControlValue->setText(checked ? QStringLiteral("1") : QStringLiteral("0"));
    });
    connect(write, &QPushButton::clicked, this, [this] {
        QString value = state().writelkValue.trimmed();
        value.remove(QLatin1Char(' '));
        value = value.toLower();
        bool valid = value.size() == 8;
        for (const QChar character : value) {
            const QChar lower = character.toLower();
            valid = valid
                && (character.isDigit() || (lower >= QLatin1Char('a') && lower <= QLatin1Char('f')));
        }
        if (!valid) {
            host()->showToast(I18n::t(QStringLiteral("锁值需要 8 位十六进制字符（4 字节，例如 80e48ed2）")),
                              Tone::Error);
            return;
        }
        const int control = state().writelkControl ? 1 : 0;
        host()->confirmAction(
            QStringLiteral("写入 UIM 锁绑定？"),
            I18n::tf(QStringLiteral("将锁值写为 {value}，控制位 {control}。写入后锁会绑定到该值，请确认卡片匹配。"),
                     {{QStringLiteral("value"), value}, {QStringLiteral("control"), control}}),
            QStringLiteral("写入"), [this, value, control] {
                host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("writelk")},
                                              {QStringLiteral("value"), value},
                                              {QStringLiteral("control"), control}},
                                  QStringLiteral("写入 UIM 锁绑定"));
            });
    });

    m_content->addWidget(frame);
}

void UimPage::buildClearCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Trash, QStringLiteral("清除绑定"),
                                   QStringLiteral("0x57：把锁值写为 ffffffff（解除绑定）"), frame));

    auto *clear = track(Ui::button(QStringLiteral("清除 UIM 锁绑定"),
                                   Ui::ButtonVariant::DangerOutlined, Icon::Trash, frame));
    Ui::usbActionHint(clear);
    body->addWidget(clear, 0, Qt::AlignLeft);
    body->addWidget(Ui::label(QStringLiteral("命令接受后会回读锁值并校验是否为 ffffffff。"),
                              QStringLiteral("subtle"), frame));

    connect(clear, &QPushButton::clicked, this, [this] {
        host()->confirmAction(
            QStringLiteral("清除 UIM 锁绑定？"),
            I18n::t(QStringLiteral("此操作会把锁值写为 ffffffff 并解除 SIM 绑定。请确认设备与卡片状态后再继续。")),
            QStringLiteral("清除绑定"), [this] {
                host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("clearlk")}},
                                  QStringLiteral("清除 UIM 锁绑定"));
            });
    });

    m_content->addWidget(frame);
}

void UimPage::buildMaintenanceCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Power, QStringLiteral("维护模式位 (NV 10481)"),
                                   QStringLiteral("0xE1 服务 + auth 4000 认证后写入"), frame));

    auto *row = new QHBoxLayout;
    row->setSpacing(8);
    m_maintControl = new ToggleSwitch(state().maintControl, frame);
    m_maintControlValue = new QLabel(frame);
    m_maintControlValue->setObjectName(QStringLiteral("monoValue"));
    m_maintControlValue->setText(QStringLiteral("control = %1")
                                     .arg(state().maintControl ? QStringLiteral("1")
                                                               : QStringLiteral("0")));
    row->addWidget(m_maintControl);
    row->addWidget(m_maintControlValue);
    row->addSpacing(12);

    auto *apply = track(
        Ui::button(QStringLiteral("应用维护位"), Ui::ButtonVariant::Outlined, Icon::Play, frame));
    Ui::usbActionHint(apply);
    row->addWidget(apply);
    row->addStretch(1);
    body->addLayout(row);
    body->addWidget(Ui::label(
        QStringLiteral("需要有效的 SN / Type2Auth 材料；认证失败时会返回 0x53 校验错误。"),
        QStringLiteral("subtle"), frame));

    connect(m_maintControl, &ToggleSwitch::toggled, this, [this](bool checked) {
        state().maintControl = checked;
        m_maintControlValue->setText(QStringLiteral("control = %1")
                                         .arg(checked ? QStringLiteral("1") : QStringLiteral("0")));
    });
    connect(apply, &QPushButton::clicked, this, [this] {
        const int control = state().maintControl ? 1 : 0;
        host()->confirmAction(
            QStringLiteral("设置维护模式位？"),
            I18n::tf(QStringLiteral("通过 0xE1 服务 + auth 4000 写入 NV 10481，control = {control}。此操作会切换维护模式位。"),
                     {{QStringLiteral("control"), control}}),
            QStringLiteral("应用"), [this, control] {
                host()->startTask(QJsonObject{{QStringLiteral("kind"),
                                               QStringLiteral("maintenance_bit")},
                                              {QStringLiteral("control"), control}},
                                  QStringLiteral("设置维护模式位"));
            });
    });

    m_content->addWidget(frame);
}

void UimPage::updateReadResult()
{
    Ui::clearLayout(m_readResultLayout);
    if (!state().hasResult)
        return;
    const QJsonObject &result = state().result;
    if (result.value(QStringLiteral("operation")).toString() != QLatin1String("readuimlk"))
        return;

    const QString value = jsonText(result, QStringLiteral("lock_value"));
    const QString control = jsonText(result, QStringLiteral("control"));
    const bool cleared = result.value(QStringLiteral("binding_cleared")).toBool(false);

    auto *chips = new QWidget(m_readResultHost);
    auto *chipLayout = new QHBoxLayout(chips);
    chipLayout->setContentsMargins(0, 0, 0, 0);
    chipLayout->setSpacing(8);
    chipLayout->addWidget(Ui::chip(I18n::t(cleared ? QStringLiteral("已解锁 / 已清除")
                                                    : QStringLiteral("已锁定")),
                                   cleared ? Tone::Success : Tone::Warning));
    chipLayout->addWidget(Ui::chip(QStringLiteral("control = %1").arg(control), Tone::Neutral));
    chipLayout->addStretch(1);
    m_readResultLayout->addWidget(chips);

    m_readResultLayout->addWidget(
        Ui::kvRow(QStringLiteral("锁值"), value.isEmpty() ? QStringLiteral("N/A") : value, true,
                  m_readResultHost));
    if (result.contains(QStringLiteral("raw_hex")))
        m_readResultLayout->addWidget(
            Ui::kvRow(QStringLiteral("原始应答"), jsonText(result, QStringLiteral("raw_hex")), true,
                      m_readResultHost));

    if (result.contains(QStringLiteral("before"))) {
        const QJsonObject before = result.value(QStringLiteral("before")).toObject();
        m_readResultLayout->addWidget(
            Ui::kvRow(QStringLiteral("写入前"), jsonText(before, QStringLiteral("lock_value")), true,
                      m_readResultHost));
    }
    if (result.contains(QStringLiteral("after"))) {
        const QJsonObject after = result.value(QStringLiteral("after")).toObject();
        m_readResultLayout->addWidget(
            Ui::kvRow(QStringLiteral("写入后"), jsonText(after, QStringLiteral("lock_value")), true,
                      m_readResultHost));
    }
    if (result.contains(QStringLiteral("verified"))) {
        const bool verified = result.value(QStringLiteral("verified")).toBool(false);
        m_readResultLayout->addWidget(
            Ui::kvRow(QStringLiteral("回读校验"),
                      I18n::t(verified ? QStringLiteral("通过") : QStringLiteral("未通过")), false,
                      m_readResultHost));
    }
}

void UimPage::refresh()
{
    if (m_writeControl != nullptr) {
        const QSignalBlocker b1(m_writeControl);
        const QSignalBlocker b2(m_maintControl);
        m_writeControl->setChecked(state().writelkControl, false);
        m_maintControl->setChecked(state().maintControl, false);
        m_writeControlValue->setText(state().writelkControl ? QStringLiteral("1") : QStringLiteral("0"));
        m_maintControlValue->setText(QStringLiteral("control = %1")
                                         .arg(state().maintControl ? QStringLiteral("1")
                                                                   : QStringLiteral("0")));
    }
    if (m_writeValue->text() != state().writelkValue) {
        const QSignalBlocker blocker(m_writeValue);
        m_writeValue->setText(state().writelkValue);
    }

    updateReadResult();
    updateLastResult();
}
