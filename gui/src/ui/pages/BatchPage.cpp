#include "ui/pages/BatchPage.h"

#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QPushButton>
#include <QVBoxLayout>

#include "core/JsonHelpers.h"
#include "ui/I18n.h"
#include "ui/Icons.h"

BatchPage::BatchPage(PageHost *pageHost, QWidget *parent)
    : Page(pageHost, parent)
{
    addPageHeader(QStringLiteral("批量查询"),
                  QStringLiteral("在单个会话内连续执行多条查询，避免反复进入 / 退出服务"));

    auto *controls = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(controls);
    body->addWidget(Ui::cardHeader(Icon::List, QStringLiteral("批量配置"),
                                   QStringLiteral("basic：版本 + 锁码；uimlk：锁状态 + record59"),
                                   controls));
    m_profile = new SegmentedControl({QStringLiteral("基础 basic"), QStringLiteral("UIMLK uimlk")},
                                     controls);
    body->addWidget(m_profile, 0, Qt::AlignLeft);

    m_run = track(Ui::button(QStringLiteral("执行批量查询"), Ui::ButtonVariant::Filled, Icon::Play,
                             controls));
    Ui::usbActionHint(m_run);
    body->addWidget(m_run, 0, Qt::AlignLeft);
    body->addWidget(Ui::label(QStringLiteral("uimlk profile 需要先完成 UIMLK 认证，SN 必须可用。"),
                              QStringLiteral("subtle"), controls));
    m_content->addWidget(controls);

    auto *resultCard = Ui::card(this);
    QVBoxLayout *resultBody = Ui::cardBody(resultCard);
    resultBody->addWidget(Ui::cardHeader(Icon::List, QStringLiteral("查询结果"),
                                         QStringLiteral("每条查询的原始字段与解码值"), resultCard));
    m_resultHost = new QWidget(resultCard);
    m_resultLayout = new QVBoxLayout(m_resultHost);
    m_resultLayout->setContentsMargins(0, 0, 0, 0);
    m_resultLayout->setSpacing(6);
    resultBody->addWidget(m_resultHost);
    m_content->addWidget(resultCard);

    addLastResultCard();

    connect(m_profile, &SegmentedControl::currentChanged, this, [this](int index) {
        state().batchProfile = index == 1 ? QStringLiteral("uimlk") : QStringLiteral("basic");
    });
    connect(m_run, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("batch")},
                                      {QStringLiteral("profile"), state().batchProfile}},
                          QStringLiteral("批量查询"));
    });
}

void BatchPage::renderResult()
{
    Ui::clearLayout(m_resultLayout);

    if (!state().hasResult
        || state().result.value(QStringLiteral("operation")).toString() != QLatin1String("batch")) {
        m_resultLayout->addWidget(Ui::emptyState(
            Icon::List, QStringLiteral("尚无批量结果"),
            QStringLiteral("选择 profile 并执行查询后，这里会列出每条结果"), m_resultHost));
        return;
    }

    const QJsonObject result = state().result;
    const QString profile = jsonText(result, QStringLiteral("profile"));

    auto *chipRow = new QWidget(m_resultHost);
    auto *chipLayout = new QHBoxLayout(chipRow);
    chipLayout->setContentsMargins(0, 0, 0, 0);
    chipLayout->addWidget(Ui::chip(QStringLiteral("profile = %1").arg(profile), Tone::Primary));
    chipLayout->addStretch(1);
    m_resultLayout->addWidget(chipRow);

    const QJsonObject error = result.value(QStringLiteral("error")).toObject();
    if (!error.isEmpty()) {
        m_resultLayout->addWidget(Ui::bannerText(
            Icon::Warning, jsonText(error, QStringLiteral("message")), Tone::Error, m_resultHost));
    }

    const QJsonArray results = result.value(QStringLiteral("results")).toArray();
    for (const QJsonValue &itemValue : results) {
        const QJsonObject item = itemValue.toObject();
        auto *title = new QLabel(
            I18n::tf(QStringLiteral("查询：{query}"),
                     {{QStringLiteral("query"), jsonText(item, QStringLiteral("query"))}}),
            m_resultHost);
        title->setObjectName(QStringLiteral("cardTitle"));
        m_resultLayout->addWidget(title);

        for (const char *key : {"decoded", "lock_value", "raw_hex", "decoded_hex"}) {
            const QString field = QString::fromLatin1(key);
            if (!item.contains(field))
                continue;
            m_resultLayout->addWidget(Ui::kvRowText(field, pyValue(item.value(field)), true,
                                                    m_resultHost));
        }
    }
}

void BatchPage::refresh()
{
    m_profile->setCurrentIndex(state().batchProfile == QLatin1String("uimlk") ? 1 : 0);
    renderResult();
    updateLastResult();
}
