#include "ui/pages/OverviewPage.h"

#include <QComboBox>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QLineEdit>
#include <QPushButton>
#include <QSignalBlocker>
#include <QVBoxLayout>

#include "ui/I18n.h"
#include "ui/Icons.h"
#include "ui/Theme.h"

namespace {

QWidget *fieldBox(QWidget *parent, const QString &labelKey, const QString &placeholderKey,
                  QLineEdit **out, int width, const QString &tooltipKey = {})
{
    auto *box = new QWidget(parent);
    auto *layout = new QVBoxLayout(box);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(3);
    layout->addWidget(Ui::label(labelKey, QStringLiteral("caption"), box));

    auto *edit = new QLineEdit(box);
    edit->setMinimumWidth(width);
    if (!placeholderKey.isEmpty()) {
        edit->setPlaceholderText(I18n::t(placeholderKey));
        edit->setProperty("i18nPlaceholder", placeholderKey);
    }
    if (!tooltipKey.isEmpty()) {
        edit->setToolTip(I18n::t(tooltipKey));
        edit->setProperty("i18nTip", tooltipKey);
    }
    layout->addWidget(edit);
    if (out != nullptr)
        *out = edit;
    return box;
}

} // namespace

OverviewPage::OverviewPage(PageHost *host, QWidget *parent)
    : Page(host, parent)
{
    addPageHeader(QStringLiteral("概览"),
                  QStringLiteral("连接 au / SERI 设备，读取锁状态、PIN 与 SIM 信息"));

    if (!host->elevated()) {
        m_content->addWidget(Ui::banner(
            Icon::Warning,
            QStringLiteral("当前以普通用户运行，USB 访问可能失败。可使用 pkexec 重新启动并完成 polkit 认证，或配置 udev 规则。"),
            Tone::Warning));
    }

    buildConnectionCard();
    buildQuickActionsCard();
    addLastResultCard();
}

void OverviewPage::buildConnectionCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Usb, QStringLiteral("设备连接"),
                                   QStringLiteral("扫描 USB 总线并选择目标设备"), frame));

    auto *row = new QHBoxLayout;
    row->setSpacing(8);
    m_scanAll = track(Ui::button(QStringLiteral("扫描全部"), Ui::ButtonVariant::Tonal, Icon::Scan, frame));
    m_scanFiltered =
        track(Ui::button(QStringLiteral("仅筛选 VID/PID"), Ui::ButtonVariant::Outlined, Icon::Probe, frame));
    row->addWidget(m_scanAll);
    row->addWidget(m_scanFiltered);

    m_deviceCombo = new QComboBox(frame);
    m_deviceCombo->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    m_deviceCombo->setMinimumWidth(300);
    row->addWidget(m_deviceCombo, 1);
    body->addLayout(row);

    connect(m_scanAll, &QPushButton::clicked, this, [this] { host()->scanDevices(false); });
    connect(m_scanFiltered, &QPushButton::clicked, this, [this] { host()->scanDevices(true); });
    connect(m_deviceCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this,
            [this](int index) {
                if (m_comboPlaceholder || index < 0)
                    return;
                host()->selectDevice(index, true);
            });

    m_scanErrorHost = new QWidget(frame);
    m_scanErrorLayout = new QVBoxLayout(m_scanErrorHost);
    m_scanErrorLayout->setContentsMargins(0, 0, 0, 0);
    m_scanErrorLayout->setSpacing(0);
    body->addWidget(m_scanErrorHost);
    m_scanErrorHost->hide();

    auto *grid = new QGridLayout;
    grid->setHorizontalSpacing(12);
    grid->setVerticalSpacing(10);
    grid->addWidget(fieldBox(frame, QStringLiteral("VID (hex)"), QStringLiteral("0482"), &m_vid, 120), 0, 0);
    grid->addWidget(fieldBox(frame, QStringLiteral("PID (hex)"), QStringLiteral("0a5c"), &m_pid, 120), 0, 1);
    grid->addWidget(fieldBox(frame, QStringLiteral("SN（可选）"),
                             QStringLiteral("SKYKA077394 或 16 位 hex"), &m_sn, 260), 0, 2);
    grid->addWidget(fieldBox(frame, QStringLiteral("接口号（可选）"), QStringLiteral("自动"), &m_interface,
                             120), 1, 0);
    grid->addWidget(fieldBox(frame, QStringLiteral("EP OUT"), QStringLiteral("04"), &m_epOut, 120), 1, 1);
    grid->addWidget(fieldBox(frame, QStringLiteral("EP IN"), QStringLiteral("84"), &m_epIn, 120), 1, 2);
    grid->setColumnStretch(2, 1);
    body->addLayout(grid);

    auto *switches = new QHBoxLayout;
    switches->setSpacing(10);
    m_type2 = new ToggleSwitch(state().type2, frame);
    switches->addWidget(m_type2);
    switches->addWidget(Ui::label(QStringLiteral("Type2Auth 52/53 认证"), {}, frame));
    switches->addSpacing(18);
    m_verbose = new ToggleSwitch(state().verbose, frame);
    switches->addWidget(m_verbose);
    switches->addWidget(Ui::label(QStringLiteral("详细 USB 日志"), {}, frame));
    switches->addStretch(1);
    body->addLayout(switches);

    body->addWidget(Ui::banner(
        Icon::Info,
        QStringLiteral("SN 留空时读取设备 iSerialNumber；若设备不提供序列号，请手动填写 11 位 SN 或 16 位十六进制。"),
        Tone::Neutral, frame));

    connect(m_type2, &ToggleSwitch::toggled, this, [this](bool checked) {
        state().type2 = checked;
        persistForm();
    });
    connect(m_verbose, &ToggleSwitch::toggled, this, [this](bool checked) {
        state().verbose = checked;
        persistForm();
    });

    const QList<QLineEdit *> edits{m_vid, m_pid, m_sn, m_interface, m_epOut, m_epIn};
    for (QLineEdit *edit : edits) {
        connect(edit, &QLineEdit::textChanged, this, [this] { persistForm(); });
    }

    m_content->addWidget(frame);
}

void OverviewPage::buildQuickActionsCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(Icon::Play, QStringLiteral("快捷操作"),
                                   QStringLiteral("所有 USB 操作都在后台线程执行，界面不会卡住"), frame));

    auto *row = new QHBoxLayout;
    row->setSpacing(8);

    auto *status = track(
        Ui::button(QStringLiteral("读取综合状态"), Ui::ButtonVariant::Filled, Icon::Dashboard, frame));
    auto *probe =
        track(Ui::button(QStringLiteral("USB 布局探测"), Ui::ButtonVariant::Tonal, Icon::Probe, frame));
    auto *exit =
        track(Ui::button(QStringLiteral("退出服务 (0xD8)"), Ui::ButtonVariant::Outlined, Icon::Power, frame));
    auto *console = Ui::button(QStringLiteral("打开日志控制台"), Ui::ButtonVariant::Text, Icon::Terminal, frame);

    Ui::usbActionHint(status);
    Ui::usbActionHint(probe);
    Ui::usbActionHint(exit);

    row->addWidget(status);
    row->addWidget(probe);
    row->addWidget(exit);
    row->addWidget(console);
    row->addStretch(1);
    body->addLayout(row);

    m_busyLabel = new QLabel(frame);
    m_busyLabel->setObjectName(QStringLiteral("subtle"));
    m_busyLabel->hide();
    body->addWidget(m_busyLabel);

    connect(status, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("status")}},
                          QStringLiteral("读取综合状态"));
    });
    connect(probe, &QPushButton::clicked, this, [this] {
        state().probeKind = QStringLiteral("list");
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("probe")},
                                      {QStringLiteral("probe"), QStringLiteral("list")},
                                      {QStringLiteral("timeout_secs"), 1.0}},
                          QStringLiteral("USB 布局探测"));
    });
    connect(exit, &QPushButton::clicked, this, [this] {
        host()->startTask(QJsonObject{{QStringLiteral("kind"), QStringLiteral("exit")}},
                          QStringLiteral("退出服务"));
    });
    connect(console, &QPushButton::clicked, this,
            [this] { host()->goToPage(PageId::Console); });

    m_content->addWidget(frame);
}

void OverviewPage::persistForm()
{
    state().vidText = m_vid->text();
    state().pidText = m_pid->text();
    state().snText = m_sn->text();
    state().interfaceText = m_interface->text();
    state().epOutText = m_epOut->text();
    state().epInText = m_epIn->text();
}

void OverviewPage::syncFormFromState()
{
    const QSignalBlocker b1(m_vid);
    const QSignalBlocker b2(m_pid);
    const QSignalBlocker b3(m_sn);
    const QSignalBlocker b4(m_interface);
    const QSignalBlocker b5(m_epOut);
    const QSignalBlocker b6(m_epIn);
    m_vid->setText(state().vidText);
    m_pid->setText(state().pidText);
    m_sn->setText(state().snText);
    m_interface->setText(state().interfaceText);
    m_epOut->setText(state().epOutText);
    m_epIn->setText(state().epInText);

    const QSignalBlocker b7(m_type2);
    const QSignalBlocker b8(m_verbose);
    m_type2->setChecked(state().type2, false);
    m_verbose->setChecked(state().verbose, false);
}

void OverviewPage::rebuildDeviceCombo()
{
    QStringList signature;
    signature.reserve(state().devices.size());
    for (const DeviceInfo &device : state().devices)
        signature.append(device.id + QLatin1Char('/') + device.display + QLatin1Char('/')
                         + device.serial);

    const QString joined = signature.join(QLatin1Char('|'))
        + QLatin1Char('#') + I18n::instance().nativeName();
    if (joined == m_deviceSignature)
        return;
    m_deviceSignature = joined;

    const QSignalBlocker blocker(m_deviceCombo);
    m_deviceCombo->clear();
    m_comboPlaceholder = state().devices.isEmpty();
    if (m_comboPlaceholder) {
        m_deviceCombo->addItem(I18n::t(QStringLiteral("未发现设备（请检查 USB 权限后重新扫描）")));
        m_deviceCombo->setCurrentIndex(-1);
        return;
    }

    for (const DeviceInfo &device : state().devices)
        m_deviceCombo->addItem(QStringLiteral("%1  ·  %2").arg(device.display, device.id));

    const int index = state().selectedDevice;
    m_deviceCombo->setCurrentIndex(index >= 0 && index < state().devices.size() ? index : 0);
}

void OverviewPage::updateScanError()
{
    Ui::clearLayout(m_scanErrorLayout);
    if (state().scanError.isEmpty()) {
        m_scanErrorHost->hide();
        return;
    }
    m_scanErrorLayout->addWidget(Ui::bannerText(Icon::Warning, state().scanError, Tone::Error,
                                                m_scanErrorHost));
    m_scanErrorHost->show();
}

void OverviewPage::refresh()
{
    rebuildDeviceCombo();

    {
        const QSignalBlocker b1(m_type2);
        const QSignalBlocker b2(m_verbose);
        m_type2->setChecked(state().type2, false);
        m_verbose->setChecked(state().verbose, false);
    }

    if (m_scanAll != nullptr) {
        m_scanAll->setText(I18n::t(state().scanning ? QStringLiteral("扫描中…")
                                                    : QStringLiteral("扫描全部")));
        m_scanAll->setProperty("i18nKey", state().scanning ? QStringLiteral("扫描中…")
                                                           : QStringLiteral("扫描全部"));
    }
    if (m_busyLabel != nullptr) {
        if (state().busy) {
            m_busyLabel->setText(I18n::tf(QStringLiteral("正在执行：{label}"),
                                          {{QStringLiteral("label"), state().busyLabel}}));
            m_busyLabel->show();
        } else {
            m_busyLabel->hide();
        }
    }

    updateScanError();
    updateLastResult();
}
