#include "ui/pages/SessionPage.h"

#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

#include "core/JsonHelpers.h"
#include "ui/I18n.h"
#include "ui/Icons.h"

SessionPage::SessionPage(PageHost *pageHost, QWidget *parent)
    : Page(pageHost, parent)
{
    addPageHeader(QStringLiteral("会话与服务"),
                  QStringLiteral("退出维护 / UIM 服务，让设备回到正常模式"));

    auto *base = Ui::card(this);
    QVBoxLayout *baseBody = Ui::cardBody(base);
    baseBody->addWidget(Ui::cardHeader(Icon::Power, QStringLiteral("退出基础服务"),
                                       QStringLiteral("发送 0xD8，恢复手机键盘 / 正常界面"),
                                       base));
    auto *exitButton = track(
        Ui::button(QStringLiteral("退出服务 (0xD8)"), Ui::ButtonVariant::Outlined, Icon::Power, base));
    Ui::usbActionHint(exitButton);
    baseBody->addWidget(exitButton, 0, Qt::AlignLeft);
    baseBody->addWidget(Ui::label(QStringLiteral("对 0FCE 设备不会发送 0xD8（与 Python 版一致）。"),
                                  QStringLiteral("subtle"), base));
    connect(exitButton, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("exit")}},
                          QStringLiteral("退出服务"));
    });
    m_content->addWidget(base);

    auto *uimlk = Ui::card(this);
    QVBoxLayout *uimlkBody = Ui::cardBody(uimlk);
    uimlkBody->addWidget(Ui::cardHeader(
        Icon::Stop, QStringLiteral("退出 UIMLK 子服务"),
        QStringLiteral("重新进入 UIMLK 并发送 0x56，设备可能重启"), uimlk));
    auto *exitUimlk = track(Ui::button(QStringLiteral("退出 UIMLK (0x56)"),
                                       Ui::ButtonVariant::DangerOutlined, Icon::Stop, uimlk));
    Ui::usbActionHint(exitUimlk);
    uimlkBody->addWidget(exitUimlk, 0, Qt::AlignLeft);
    uimlkBody->addWidget(Ui::label(
        QStringLiteral("返回结果中的 exit 字段会记录 acknowledged / reply / may_reboot。"),
        QStringLiteral("subtle"), uimlk));
    connect(exitUimlk, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("exituimlk")}},
                          QStringLiteral("退出 UIMLK"));
    });
    m_content->addWidget(uimlk);

    auto *details = Ui::card(this);
    QVBoxLayout *detailsBody = Ui::cardBody(details);
    detailsBody->addWidget(Ui::cardHeader(Icon::Info, QStringLiteral("会话详情"),
                                          QStringLiteral("退出应答与异步事件"), details));
    m_detailsHost = new QWidget(details);
    m_detailsLayout = new QVBoxLayout(m_detailsHost);
    m_detailsLayout->setContentsMargins(0, 0, 0, 0);
    m_detailsLayout->setSpacing(6);
    detailsBody->addWidget(m_detailsHost);
    m_content->addWidget(details);

    addLastResultCard();
}

void SessionPage::renderDetails()
{
    Ui::clearLayout(m_detailsLayout);

    if (!state().hasResult) {
        m_detailsLayout->addWidget(Ui::emptyState(
            Icon::Power, QStringLiteral("还没有会话记录"),
            QStringLiteral("执行退出服务后，应答信息会显示在这里"), m_detailsHost));
        return;
    }

    const QJsonObject exit = state().result.value(QStringLiteral("exit")).toObject();
    if (exit.isEmpty()) {
        m_detailsLayout->addWidget(Ui::label(
            QStringLiteral("本次操作没有产生服务退出记录。"), QStringLiteral("subtle"),
            m_detailsHost));
    } else {
        const bool acknowledged = exit.value(QStringLiteral("acknowledged")).toBool(false);
        const bool mayReboot = exit.value(QStringLiteral("may_reboot")).toBool(false);

        auto *chips = new QWidget(m_detailsHost);
        auto *chipLayout = new QHBoxLayout(chips);
        chipLayout->setContentsMargins(0, 0, 0, 0);
        chipLayout->setSpacing(8);
        chipLayout->addWidget(Ui::chip(
            I18n::t(acknowledged ? QStringLiteral("退出已确认") : QStringLiteral("退出未确认")),
            acknowledged ? Tone::Success : Tone::Warning));
        if (mayReboot)
            chipLayout->addWidget(Ui::chip(I18n::t(QStringLiteral("可能重启")), Tone::Warning));
        chipLayout->addStretch(1);
        m_detailsLayout->addWidget(chips);

        if (exit.contains(QStringLiteral("reply")))
            m_detailsLayout->addWidget(Ui::kvRow(QStringLiteral("应答"),
                                                 jsonText(exit, QStringLiteral("reply")), true,
                                                 m_detailsHost));
        if (exit.contains(QStringLiteral("error")))
            m_detailsLayout->addWidget(Ui::kvRow(QStringLiteral("错误"),
                                                 jsonText(exit, QStringLiteral("error")), false,
                                                 m_detailsHost));
    }

    const QJsonArray events = state().result.value(QStringLiteral("async_events")).toArray();
    if (!events.isEmpty()) {
        m_detailsLayout->addWidget(Ui::label(QStringLiteral("异步事件"),
                                             QStringLiteral("subtle"), m_detailsHost));
        for (const QJsonValue &eventValue : events) {
            const QJsonObject event = eventValue.toObject();
            const QString opcode = jsonText(event, QStringLiteral("opcode"));
            m_detailsLayout->addWidget(
                Ui::kvRowText(QStringLiteral("opcode %1").arg(opcode),
                              jsonText(event, QStringLiteral("meaning")), true, m_detailsHost));
        }
    }
}

void SessionPage::refresh()
{
    renderDetails();
    updateLastResult();
}
