#include "ui/pages/ProbePage.h"

#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QSignalBlocker>
#include <QVBoxLayout>

#include "core/JsonHelpers.h"
#include "ui/I18n.h"
#include "ui/Icons.h"

ProbePage::ProbePage(PageHost *host, QWidget *parent)
    : Page(host, parent)
{
    addPageHeader(QStringLiteral("探测"),
                  QStringLiteral("枚举 USB 布局，或主动探测 SERI / SCDP 服务"));

    auto *controls = Ui::card(this);
    QVBoxLayout *controlsBody = Ui::cardBody(controls);
    controlsBody->addWidget(Ui::cardHeader(
        Icon::Probe, QStringLiteral("探测模式"),
        QStringLiteral("list 只枚举配置；seri / scdp 会实际收发 USB 数据"), controls));

    m_mode = new SegmentedControl({QStringLiteral("布局 list"), QStringLiteral("SERI 回声"),
                                   QStringLiteral("SCDP")},
                                  controls);
    controlsBody->addWidget(m_mode, 0, Qt::AlignLeft);

    auto *row = new QHBoxLayout;
    row->setSpacing(12);
    auto *timeoutBox = new QWidget(controls);
    auto *timeoutLayout = new QVBoxLayout(timeoutBox);
    timeoutLayout->setContentsMargins(0, 0, 0, 0);
    timeoutLayout->setSpacing(3);
    timeoutLayout->addWidget(
        Ui::label(QStringLiteral("探测超时（秒）"), QStringLiteral("caption"), timeoutBox));
    m_timeout = new QLineEdit(state().probeTimeout, timeoutBox);
    m_timeout->setMinimumWidth(160);
    timeoutLayout->addWidget(m_timeout);
    row->addWidget(timeoutBox);

    m_run = track(Ui::button(QStringLiteral("开始探测"), Ui::ButtonVariant::Filled, Icon::Play,
                             controls));
    Ui::usbActionHint(m_run);
    row->addWidget(m_run, 0, Qt::AlignBottom);
    row->addStretch(1);
    controlsBody->addLayout(row);
    controlsBody->addWidget(Ui::label(
        QStringLiteral("SERI 探测会发送 1C 05；非 0FCE 设备在成功后自动发送 D8 清理。"),
        QStringLiteral("subtle"), controls));
    m_content->addWidget(controls);

    auto *resultCard = Ui::card(this);
    QVBoxLayout *resultBody = Ui::cardBody(resultCard);
    resultBody->addWidget(Ui::cardHeader(Icon::Usb, QStringLiteral("USB 布局"),
                                         QStringLiteral("配置 / 接口 / 端点与候选 bulk 端点对"),
                                         resultCard));
    m_resultHost = new QWidget(resultCard);
    m_resultLayout = new QVBoxLayout(m_resultHost);
    m_resultLayout->setContentsMargins(0, 0, 0, 0);
    m_resultLayout->setSpacing(6);
    resultBody->addWidget(m_resultHost);
    m_content->addWidget(resultCard);

    addLastResultCard();

    connect(m_mode, &SegmentedControl::currentChanged, this, [this](int index) {
        static const char *kinds[] = {"list", "seri", "scdp"};
        state().probeKind = QString::fromLatin1(kinds[qBound(0, index, 2)]);
        m_timeout->setEnabled(state().probeKind != QLatin1String("list"));
    });
    connect(m_timeout, &QLineEdit::textChanged, this,
            [this](const QString &text) { state().probeTimeout = text; });
    connect(m_run, &QPushButton::clicked, this, [this] { startProbe(); });
}

void ProbePage::startProbe()
{
    bool ok = false;
    const double timeout = state().probeTimeout.trimmed().toDouble(&ok);
    if (!ok || !(timeout > 0.0)) {
        host()->showToast(I18n::t(QStringLiteral("探测超时必须大于 0 秒")), Tone::Error);
        return;
    }

    const QString kind = state().probeKind;
    QString label = QStringLiteral("USB 布局探测");
    if (kind == QLatin1String("seri"))
        label = QStringLiteral("SERI 回声探测");
    else if (kind == QLatin1String("scdp"))
        label = QStringLiteral("SCDP 探测");

    host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("probe")},
                                  {QStringLiteral("probe"), kind},
                                  {QStringLiteral("timeout_secs"), timeout}},
                      label);
}

void ProbePage::renderProbe(const QJsonObject &result)
{
    const QString kind = jsonText(result, QStringLiteral("probe"));

    auto *chips = new QWidget(m_resultHost);
    auto *chipLayout = new QHBoxLayout(chips);
    chipLayout->setContentsMargins(0, 0, 0, 0);
    chipLayout->setSpacing(8);
    chipLayout->addWidget(Ui::chip(QStringLiteral("probe = %1").arg(kind), Tone::Primary));
    if (result.value(QStringLiteral("matched")).toBool(false))
        chipLayout->addWidget(Ui::chip(I18n::t(QStringLiteral("SERI 回声匹配")), Tone::Success));
    if (result.value(QStringLiteral("responsive")).toBool(false))
        chipLayout->addWidget(Ui::chip(I18n::t(QStringLiteral("设备有响应")), Tone::Success));
    chipLayout->addStretch(1);
    m_resultLayout->addWidget(chips);

    const QJsonObject selection = result.value(QStringLiteral("selection")).toObject();
    if (!selection.isEmpty()) {
        const QString interfaceNumber = jsonText(selection, QStringLiteral("interface"));
        if (!interfaceNumber.isEmpty() && interfaceNumber != QLatin1String("None")) {
            const QString alt = jsonText(selection, QStringLiteral("alternate_setting"));
            const QString epOut = jsonText(selection, QStringLiteral("ep_out"));
            const QString epIn = jsonText(selection, QStringLiteral("ep_in"));
            m_resultLayout->addWidget(Ui::kvRowText(
                I18n::t(QStringLiteral("选中接口")),
                QStringLiteral("if %1 alt %2 · OUT %3 / IN %4")
                    .arg(interfaceNumber, alt, epOut, epIn),
                true, m_resultHost));
        }
    }

    if (result.contains(QStringLiteral("request_hex")))
        m_resultLayout->addWidget(Ui::kvRow(QStringLiteral("请求"),
                                            jsonText(result, QStringLiteral("request_hex")), true,
                                            m_resultHost));
    if (result.contains(QStringLiteral("response_hex"))) {
        QString response = jsonText(result, QStringLiteral("response_hex"));
        if (response.isEmpty())
            response = I18n::t(QStringLiteral("<超时>"));
        m_resultLayout->addWidget(
            Ui::kvRow(QStringLiteral("响应"), response, true, m_resultHost));
    }
    if (result.contains(QStringLiteral("cleanup_hex")))
        m_resultLayout->addWidget(Ui::kvRow(QStringLiteral("清理命令"),
                                            jsonText(result, QStringLiteral("cleanup_hex")), true,
                                            m_resultHost));

    const QJsonObject usb = result.value(QStringLiteral("usb")).toObject();
    if (usb.isEmpty())
        return;

    auto *device = new QLabel(
        I18n::tf(QStringLiteral("设备 {vid}:{pid}"),
                 {{QStringLiteral("vid"), jsonText(usb, QStringLiteral("vid"))},
                  {QStringLiteral("pid"), jsonText(usb, QStringLiteral("pid"))}}),
        m_resultHost);
    device->setObjectName(QStringLiteral("subtle"));
    m_resultLayout->addWidget(device);

    const QJsonArray configurations = usb.value(QStringLiteral("configurations")).toArray();
    for (const QJsonValue &configValue : configurations) {
        const QJsonObject config = configValue.toObject();
        const QString value = jsonText(config, QStringLiteral("value"));
        const bool active = config.value(QStringLiteral("active")).toBool(false);

        auto *configRow = new QWidget(m_resultHost);
        auto *configLayout = new QHBoxLayout(configRow);
        configLayout->setContentsMargins(0, 4, 0, 0);
        configLayout->setSpacing(8);
        configLayout->addWidget(Ui::chip(
            I18n::tf(QStringLiteral("配置 {value}"), {{QStringLiteral("value"), value}}),
            active ? Tone::Primary : Tone::Neutral));
        if (active)
            configLayout->addWidget(Ui::chip(I18n::t(QStringLiteral("活动")), Tone::Success));
        configLayout->addStretch(1);
        m_resultLayout->addWidget(configRow);

        const QJsonArray interfaces = config.value(QStringLiteral("interfaces")).toArray();
        for (const QJsonValue &interfaceValue : interfaces) {
            const QJsonObject interface = interfaceValue.toObject();
            const QString number = jsonText(interface, QStringLiteral("number"));
            const QString alt = jsonText(interface, QStringLiteral("alternate_setting"));
            const QString className = jsonText(interface, QStringLiteral("class"));
            const QString subclass = jsonText(interface, QStringLiteral("subclass"));
            const QString protocol = jsonText(interface, QStringLiteral("protocol"));

            auto *title = new QLabel(
                I18n::tf(QStringLiteral("接口 {number} · alt {alt}"),
                         {{QStringLiteral("number"), number}, {QStringLiteral("alt"), alt}}),
                m_resultHost);
            title->setObjectName(QStringLiteral("cardTitle"));
            m_resultLayout->addWidget(title);

            auto *classLabel = new QLabel(
                QStringLiteral("class %1/%2/%3").arg(className, subclass, protocol), m_resultHost);
            classLabel->setObjectName(QStringLiteral("monoValue"));
            m_resultLayout->addWidget(classLabel);

            const QJsonArray endpoints = interface.value(QStringLiteral("endpoints")).toArray();
            for (const QJsonValue &endpointValue : endpoints) {
                const QJsonObject endpoint = endpointValue.toObject();
                auto *endpointLabel = new QLabel(
                    QStringLiteral("    %1 %2 · %3 · %4 B")
                        .arg(jsonText(endpoint, QStringLiteral("address")),
                             jsonText(endpoint, QStringLiteral("direction")),
                             jsonText(endpoint, QStringLiteral("type")),
                             jsonText(endpoint, QStringLiteral("max_packet_size"))),
                    m_resultHost);
                endpointLabel->setObjectName(QStringLiteral("monoValue"));
                m_resultLayout->addWidget(endpointLabel);
            }
        }
    }

    const QJsonArray candidates = usb.value(QStringLiteral("bulk_candidates")).toArray();
    if (candidates.isEmpty()) {
        m_resultLayout->addWidget(Ui::label(QStringLiteral("未发现 bulk 端点对"),
                                            QStringLiteral("subtle"), m_resultHost));
    } else {
        m_resultLayout->addWidget(Ui::label(QStringLiteral("候选 bulk 端点对"),
                                            QStringLiteral("subtle"), m_resultHost));
        for (const QJsonValue &candidateValue : candidates) {
            const QJsonObject candidate = candidateValue.toObject();
            auto *row = new QWidget(m_resultHost);
            auto *layout = new QHBoxLayout(row);
            layout->setContentsMargins(0, 0, 0, 0);
            layout->setSpacing(8);

            auto *text = new QLabel(
                QStringLiteral("cfg %1 · if %2 alt %3 · OUT %4 / IN %5")
                    .arg(jsonText(candidate, QStringLiteral("configuration")),
                         jsonText(candidate, QStringLiteral("interface")),
                         jsonText(candidate, QStringLiteral("alternate_setting")),
                         jsonText(candidate, QStringLiteral("ep_out")),
                         jsonText(candidate, QStringLiteral("ep_in"))),
                row);
            text->setObjectName(QStringLiteral("monoValue"));
            layout->addWidget(text);
            if (candidate.value(QStringLiteral("active")).toBool(false))
                layout->addWidget(Ui::chip(I18n::t(QStringLiteral("活动")), Tone::Success));
            layout->addStretch(1);
            m_resultLayout->addWidget(row);
        }
    }
}

void ProbePage::renderResult()
{
    Ui::clearLayout(m_resultLayout);
    if (!state().hasResult
        || state().result.value(QStringLiteral("operation")).toString() != QLatin1String("probe")) {
        m_resultLayout->addWidget(Ui::emptyState(
            Icon::Usb, QStringLiteral("尚无探测结果"),
            QStringLiteral("运行布局探测后，这里会显示 USB 配置树与端点信息"), m_resultHost));
        return;
    }
    renderProbe(state().result);
}

void ProbePage::refresh()
{
    if (m_timeout->text() != state().probeTimeout) {
        const QSignalBlocker blocker(m_timeout);
        m_timeout->setText(state().probeTimeout);
    }
    m_timeout->setEnabled(state().probeKind != QLatin1String("list"));
    const int index = state().probeKind == QLatin1String("seri") ? 1
        : state().probeKind == QLatin1String("scdp")          ? 2
                                                              : 0;
    m_mode->setCurrentIndex(index);

    renderResult();
    updateLastResult();
}
