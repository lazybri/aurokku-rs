#include "Page.h"

#include <QGridLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

#include "core/JsonHelpers.h"
#include "ui/I18n.h"
#include "ui/Icons.h"
#include "ui/Theme.h"

Page::Page(PageHost *host, QWidget *parent)
    : QWidget(parent)
    , m_host(host)
{
    setObjectName(QStringLiteral("pageRoot"));
    m_content = new QVBoxLayout(this);
    m_content->setContentsMargins(24, 20, 24, 20);
    m_content->setSpacing(12);
}

void Page::retranslate()
{
    Ui::retranslateTree(this);
}

void Page::setBusy(bool busy)
{
    for (QPushButton *button : m_actions)
        button->setEnabled(!busy);
}

QPushButton *Page::track(QPushButton *button)
{
    m_actions.append(button);
    return button;
}

void Page::addPageHeader(const QString &titleKey, const QString &subtitleKey)
{
    auto *header = new QWidget(this);
    auto *layout = new QVBoxLayout(header);
    layout->setContentsMargins(0, 0, 0, 4);
    layout->setSpacing(2);
    layout->addWidget(Ui::label(titleKey, QStringLiteral("pageTitle"), header));
    layout->addWidget(Ui::label(subtitleKey, QStringLiteral("pageSubtitle"), header));
    m_content->addWidget(header);
}

void Page::addLastResultCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(
        Ui::cardHeader(Icon::Info, QStringLiteral("最近结果"), QStringLiteral("最近一次操作的结构化摘要"), frame));

    m_lastResultBody = new QVBoxLayout;
    m_lastResultBody->setContentsMargins(0, 0, 0, 0);
    m_lastResultBody->setSpacing(6);
    body->addLayout(m_lastResultBody);

    m_content->addWidget(frame);
    m_content->addStretch(1);
}

void Page::updateLastResult()
{
    if (m_lastResultBody == nullptr)
        return;

    Ui::clearLayout(m_lastResultBody);

    if (!state().lastErrorKind.isEmpty()) {
        m_lastResultBody->addWidget(Ui::bannerText(
            Icon::Warning,
            QStringLiteral("%1: %2").arg(state().lastErrorKind, state().lastErrorMessage),
            Tone::Error));
        return;
    }

    if (!state().hasResult) {
        m_lastResultBody->addWidget(Ui::emptyState(
            Icon::Info, QStringLiteral("还没有结果"),
            QStringLiteral("运行一次状态读取或探测后，结果会显示在这里")));
        return;
    }

    auto *chips = new QWidget;
    auto *chipLayout = new QHBoxLayout(chips);
    chipLayout->setContentsMargins(0, 0, 0, 0);
    chipLayout->setSpacing(8);
    chipLayout->addWidget(Ui::chip(I18n::t(state().lastSuccess ? QStringLiteral("成功")
                                                               : QStringLiteral("失败")),
                                   state().lastSuccess ? Tone::Success : Tone::Warning));
    auto *op = new QLabel(state().lastOp);
    op->setObjectName(QStringLiteral("monoValue"));
    chipLayout->addWidget(op);
    chipLayout->addStretch(1);
    m_lastResultBody->addWidget(chips);

    const QString operation = state().result.value(QStringLiteral("operation")).toString();
    if (state().lastSuccess && operation == QLatin1String("status"))
        m_lastResultBody->addWidget(buildStatusSummary(state().result));
    else
        m_lastResultBody->addWidget(buildGenericSummary(state().result));

    auto *json = Ui::button(QStringLiteral("查看完整 JSON"), Ui::ButtonVariant::Text, Icon::Terminal);
    connect(json, &QPushButton::clicked, this, [this] { host()->goToPage(PageId::Console); });
    m_lastResultBody->addWidget(json, 0, Qt::AlignLeft);
}

QWidget *Page::buildStatusSummary(const QJsonObject &result)
{
    auto *widget = new QWidget;
    auto *grid = new QGridLayout(widget);
    grid->setContentsMargins(0, 0, 0, 0);
    grid->setHorizontalSpacing(10);
    grid->setVerticalSpacing(10);

    const QString sn = jsonText(result, QStringLiteral("sn"));
    const QString pin = jsonText(result, QStringLiteral("lockcode"));
    const QString version = jsonText(result, QStringLiteral("version"));
    const QString iccid = jsonText(result, QStringLiteral("iccid_record"));

    const QJsonObject uim = jsonObject(result, QStringLiteral("uim_lock"));
    const bool unlocked = uim.value(QStringLiteral("unlocked")).toBool(false);
    const QString lockValue = uim.value(QStringLiteral("lock_value")).toString(QStringLiteral("N/A"));
    const QString uimText = unlocked
        ? I18n::t(QStringLiteral("已解锁 (ffffffff)"))
        : I18n::tf(QStringLiteral("已锁定 ({value})"), {{QStringLiteral("value"), lockValue}});

    grid->addWidget(Ui::metricText(Icon::Usb, I18n::t(QStringLiteral("设备 SN")),
                                   sn.isEmpty() ? QStringLiteral("N/A") : sn, Tone::Primary, widget),
                    0, 0);
    grid->addWidget(Ui::metricText(Icon::Key, I18n::t(QStringLiteral("手机 PIN")),
                                   pin.isEmpty() ? QStringLiteral("N/A") : pin, Tone::Tertiary, widget),
                    0, 1);
    grid->addWidget(Ui::metricText(Icon::Lock, I18n::t(QStringLiteral("UIM 锁")), uimText,
                                   unlocked ? Tone::Success : Tone::Warning, widget),
                    1, 0);
    grid->addWidget(Ui::metricText(Icon::List, QStringLiteral("ICCID"),
                                   iccid.isEmpty() ? QStringLiteral("N/A") : iccid, Tone::Neutral,
                                   widget),
                    1, 1);
    grid->addWidget(Ui::kvRowText(I18n::t(QStringLiteral("Base 版本 (raw)")),
                                  version.isEmpty() ? QStringLiteral("N/A") : version, true, widget),
                    2, 0, 1, 2);
    return widget;
}

QWidget *Page::buildGenericSummary(const QJsonObject &result)
{
    auto *widget = new QWidget;
    auto *layout = new QVBoxLayout(widget);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(4);

    struct KeyLabel {
        const char *key;
        const char *label;
    };
    static const KeyLabel keys[] = {
        {"lock_value", "UIM 锁值"},   {"control", "控制位"},
        {"lockcode", "手机 PIN"},     {"decoded_hex", "解码数据"},
        {"raw_hex", "原始数据"},       {"reply", "应答"},
        {"request_hex", "请求"},       {"response_hex", "响应"},
        {"maintenance_bit", "维护位"}, {"method", "退出方式"},
    };

    int shown = 0;
    for (const auto &entry : keys) {
        const QString key = QString::fromLatin1(entry.key);
        if (!result.contains(key))
            continue;
        const QString text = pyValue(result.value(key));
        if (text.isEmpty() || text == QLatin1String("None"))
            continue;
        layout->addWidget(
            Ui::kvRow(QString::fromUtf8(entry.label), text, true, widget));
        ++shown;
    }

    if (result.contains(QStringLiteral("verified"))) {
        const bool verified = result.value(QStringLiteral("verified")).toBool(false);
        layout->addWidget(Ui::kvRow(QStringLiteral("写入校验"),
                                    I18n::t(verified ? QStringLiteral("通过")
                                                     : QStringLiteral("未通过")),
                                    false, widget));
        ++shown;
    }
    if (result.contains(QStringLiteral("command_accepted"))) {
        const bool accepted = result.value(QStringLiteral("command_accepted")).toBool(false);
        layout->addWidget(Ui::kvRow(QStringLiteral("命令接受"),
                                    I18n::t(accepted ? QStringLiteral("是") : QStringLiteral("否")),
                                    false, widget));
        ++shown;
    }

    if (shown == 0) {
        layout->addWidget(Ui::label(
            QStringLiteral("该操作没有可展开的摘要字段，请查看完整 JSON。"),
            QStringLiteral("subtle"), widget));
    }
    return widget;
}
