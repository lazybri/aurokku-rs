#include "ui/pages/PinPage.h"

#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

#include "core/JsonHelpers.h"
#include "ui/I18n.h"
#include "ui/Icons.h"

namespace {

QWidget *resultHost(QWidget *parent, QVBoxLayout **layoutOut)
{
    auto *host = new QWidget(parent);
    auto *layout = new QVBoxLayout(host);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(6);
    *layoutOut = layout;
    return host;
}

} // namespace

PinPage::PinPage(PageHost *host, QWidget *parent)
    : Page(host, parent)
{
    addPageHeader(QStringLiteral("PIN / ICCID"),
                  QStringLiteral("读取或重置手机锁码（NV 82），读取 ICCID record 0x59"));
    buildReadCard();
    buildResetCard();
    buildIccidCard();
    addLastResultCard();
}

void PinPage::buildReadCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Key, QStringLiteral("读取手机锁码"),
                                   QStringLiteral("SERI 0xDC 0x00，按 0xAA 终止符解析"), frame));

    auto *button =
        track(Ui::button(QStringLiteral("读取 PIN"), Ui::ButtonVariant::Filled, Icon::Scan, frame));
    Ui::usbActionHint(button);
    body->addWidget(button, 0, Qt::AlignLeft);
    connect(button, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("readpin")}},
                          QStringLiteral("读取手机锁码"));
    });

    body->addWidget(resultHost(frame, &m_readLayout));
    m_readHost = m_readLayout->parentWidget();
    m_content->addWidget(frame);
}

void PinPage::buildResetCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Refresh, QStringLiteral("重置手机锁码"),
                                   QStringLiteral("SERI 0xDC 0x01，恢复默认值 1234 并回读校验"), frame));

    auto *button = track(Ui::button(QStringLiteral("重置 PIN 为 1234"),
                                    Ui::ButtonVariant::DangerOutlined, Icon::Refresh, frame));
    Ui::usbActionHint(button);
    body->addWidget(button, 0, Qt::AlignLeft);
    connect(button, &QPushButton::clicked, this, [this] {
        host()->confirmAction(
            QStringLiteral("重置手机锁码？"),
            I18n::t(QStringLiteral("手机锁码（PIN / NV 82）将恢复为默认值 1234。")),
            QStringLiteral("重置 PIN"), [this] {
                host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("resetpin")}},
                                  QStringLiteral("重置手机锁码"));
            });
    });

    body->addWidget(resultHost(frame, &m_resetLayout));
    m_resetHost = m_resetLayout->parentWidget();
    m_content->addWidget(frame);
}

void PinPage::buildIccidCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::List, QStringLiteral("ICCID 记录"),
                                   QStringLiteral("进入 UIMLK 会话读取 record 0x59"), frame));

    auto *button =
        track(Ui::button(QStringLiteral("读取 ICCID"), Ui::ButtonVariant::Tonal, Icon::Scan, frame));
    Ui::usbActionHint(button);
    body->addWidget(button, 0, Qt::AlignLeft);
    connect(button, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("readiccid")}},
                          QStringLiteral("读取 ICCID"));
    });

    body->addWidget(resultHost(frame, &m_iccidLayout));
    m_iccidHost = m_iccidLayout->parentWidget();
    m_content->addWidget(frame);
}

void PinPage::updateCards()
{
    Ui::clearLayout(m_readLayout);
    Ui::clearLayout(m_resetLayout);
    Ui::clearLayout(m_iccidLayout);

    if (!state().hasResult)
        return;

    const QJsonObject &result = state().result;
    const QString operation = result.value(QStringLiteral("operation")).toString();

    if (operation == QLatin1String("readpin")) {
        const QString pin = jsonText(result, QStringLiteral("lockcode"));
        m_readLayout->addWidget(Ui::metricText(Icon::Key, I18n::t(QStringLiteral("手机锁码")),
                                                pin.isEmpty() ? QStringLiteral("N/A") : pin,
                                                Tone::Tertiary, m_readHost));
        if (result.contains(QStringLiteral("raw_hex"))) {
            m_readLayout->addWidget(Ui::kvRow(QStringLiteral("原始应答"),
                                              jsonText(result, QStringLiteral("raw_hex")), true,
                                              m_readHost));
        }
    } else if (operation == QLatin1String("resetpin")) {
        const QString pin = jsonText(result, QStringLiteral("lockcode"));
        const bool verified = result.value(QStringLiteral("verified")).toBool(false);
        m_resetLayout->addWidget(Ui::metricText(Icon::Key, I18n::t(QStringLiteral("回读 PIN")),
                                                 pin.isEmpty() ? QStringLiteral("N/A") : pin,
                                                 verified ? Tone::Success : Tone::Warning,
                                                 m_resetHost));
        m_resetLayout->addWidget(
            Ui::kvRow(QStringLiteral("1234 校验"),
                      I18n::t(verified ? QStringLiteral("通过") : QStringLiteral("未通过")), false,
                      m_resetHost));
    } else if (operation == QLatin1String("readiccid")) {
        const QString iccid = jsonText(result, QStringLiteral("decoded_hex"));
        m_iccidLayout->addWidget(Ui::metricText(Icon::List, I18n::t(QStringLiteral("解码 ICCID")),
                                                 iccid.isEmpty() ? QStringLiteral("N/A") : iccid,
                                                 Tone::Primary, m_iccidHost));
        if (result.contains(QStringLiteral("raw_hex"))) {
            m_iccidLayout->addWidget(Ui::kvRow(QStringLiteral("原始应答"),
                                               jsonText(result, QStringLiteral("raw_hex")), true,
                                               m_iccidHost));
        }
    }
}

void PinPage::refresh()
{
    updateCards();
    updateLastResult();
}
