#include "ui/pages/OverviewPage.h"

#include <QClipboard>
#include <QComboBox>
#include <QGridLayout>
#include <QGuiApplication>
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

#if defined(Q_OS_LINUX) || defined(__linux__)
    m_content->addWidget(Ui::banner(
        Icon::Info,
        QStringLiteral("扫描后选择设备将请求授权，并仅为当前 USB 设备临时添加读写 ACL；更换设备、重新扫描或退出软件时会恢复原规则。"),
        Tone::Neutral));
#endif

    buildConnectionCard();
    buildQuickActionsCard();
    buildDriverCard();
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
                host()->selectDevice(index - 1, true);
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

void OverviewPage::buildDriverCard()
{
    auto *frame = Ui::card(this);
    QVBoxLayout *body = Ui::cardBody(frame);
    body->addWidget(Ui::cardHeader(
        Icon::Usb, QStringLiteral("WinUSB 驱动"),
        QStringLiteral("Windows 上设备默认由 usbser.sys 占用，需要安装 WinUSB 驱动才能访问"),
        frame));
    body->addWidget(Ui::banner(
        Icon::Info,
        QStringLiteral("该设备当前由 usbser.sys 占用；libusb 需要 WinUSB 驱动才能访问。安装时需要确认一次 UAC。"),
        Tone::Neutral, frame));

    auto *row = new QHBoxLayout;
    row->setSpacing(8);
    m_installDriver = track(Ui::button(QStringLiteral("安装 WinUSB 驱动"),
                                       Ui::ButtonVariant::Filled, Icon::Usb, frame));
    m_restoreDriver = track(Ui::button(QStringLiteral("恢复原驱动"),
                                       Ui::ButtonVariant::Outlined, Icon::Refresh, frame));
    m_copyLog = Ui::button(QStringLiteral("复制路径"), Ui::ButtonVariant::Text, Icon::Copy, frame);
    row->addWidget(m_installDriver);
    row->addWidget(m_restoreDriver);
    row->addWidget(m_copyLog);
    row->addStretch(1);
    body->addLayout(row);

    auto *toggleRow = new QHBoxLayout;
    toggleRow->setSpacing(10);
    m_restoreOnExit = new ToggleSwitch(state().restoreDriverOnExit, frame);
    toggleRow->addWidget(m_restoreOnExit);
    toggleRow->addWidget(Ui::label(QStringLiteral("退出时自动恢复原驱动"), {}, frame));
    toggleRow->addStretch(1);
    body->addLayout(toggleRow);

    m_driverState = Ui::valueLabel(QString(), false, frame);
    m_driverState->setWordWrap(true);
    body->addWidget(m_driverState);
    m_driverService = Ui::valueLabel(QString(), true, frame);
    body->addWidget(m_driverService);
    m_driverLog = Ui::valueLabel(QString(), true, frame);
    m_driverLog->setWordWrap(true);
    body->addWidget(m_driverLog);

    connect(m_installDriver, &QPushButton::clicked, this, [this] {
        quint16 vid = 0;
        quint16 pid = 0;
        if (!driverTarget(&vid, &pid)) {
            host()->showToast(I18n::t(QStringLiteral("请先扫描并选择 USB 设备")), Tone::Warning);
            return;
        }
        host()->installDriver(vid, pid, m_restoreOnExit->isChecked());
    });
    connect(m_restoreDriver, &QPushButton::clicked, this, [this] {
        quint16 vid = 0;
        quint16 pid = 0;
        if (!driverTarget(&vid, &pid)) {
            host()->showToast(I18n::t(QStringLiteral("请先扫描并选择 USB 设备")), Tone::Warning);
            return;
        }
        host()->restoreDriver(vid, pid);
    });
    connect(m_copyLog, &QPushButton::clicked, this, [this] {
        if (state().driver.logPath.isEmpty())
            return;
        QGuiApplication::clipboard()->setText(state().driver.logPath);
        host()->showToast(I18n::t(QStringLiteral("已复制驱动日志路径")), Tone::Primary);
    });
    connect(m_restoreOnExit, &ToggleSwitch::toggled, this, [this](bool checked) {
        state().restoreDriverOnExit = checked;
    });

    m_driverCard = frame;
    frame->hide();
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
                         + device.serial + QLatin1Char('/') + QString::number(device.bus)
                         + QLatin1Char('/') + QString::number(device.address));

    const QString joined = signature.join(QLatin1Char('|'))
        + QLatin1Char('#') + I18n::instance().nativeName();
    if (joined == m_deviceSignature)
        return;
    m_deviceSignature = joined;

    const QSignalBlocker blocker(m_deviceCombo);
    m_deviceCombo->clear();
    m_comboPlaceholder = state().devices.isEmpty();
    m_deviceCombo->addItem(I18n::t(m_comboPlaceholder
                                       ? QStringLiteral("未发现设备（请扫描 USB 设备）")
                                       : QStringLiteral("请选择 USB 设备")));

    for (const DeviceInfo &device : state().devices)
        m_deviceCombo->addItem(QStringLiteral("%1  ·  %2").arg(device.display, device.id));

    const int index = state().selectedDevice;
    m_deviceCombo->setCurrentIndex(index >= 0 && index < state().devices.size() ? index + 1 : 0);
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

bool OverviewPage::driverTarget(quint16 *vid, quint16 *pid) const
{
    if (state().driver.pending) {
        if (vid != nullptr)
            *vid = state().driver.pendingVid;
        if (pid != nullptr)
            *pid = state().driver.pendingPid;
        return true;
    }
    if (state().hasDriverTarget) {
        if (vid != nullptr)
            *vid = state().driverTargetVid;
        if (pid != nullptr)
            *pid = state().driverTargetPid;
        return true;
    }
    const int index = state().selectedDevice;
    if (index >= 0 && index < state().devices.size()) {
        if (vid != nullptr)
            *vid = state().devices.at(index).vid;
        if (pid != nullptr)
            *pid = state().devices.at(index).pid;
        return true;
    }
    return false;
}

void OverviewPage::refreshDriverCard()
{
    if (m_driverCard == nullptr)
        return;

    const bool visible = state().driver.supported;
    m_driverCard->setVisible(visible);
    if (!visible)
        return;

    quint16 vid = 0;
    quint16 pid = 0;
    const bool hasTarget = driverTarget(&vid, &pid);

    {
        const QSignalBlocker blocker(m_restoreOnExit);
        m_restoreOnExit->setChecked(state().restoreDriverOnExit, false);
    }

    QString stateText;
    if (state().driver.pending) {
        stateText = I18n::tf(
            QStringLiteral("上次会话未恢复 {target} 的原始 USB 驱动，可在概览页的 WinUSB 驱动卡片中恢复"),
            {{QStringLiteral("target"), state().driver.pendingTarget()}});
    } else if (state().driver.busy) {
        stateText = I18n::t(QStringLiteral("正在处理 WinUSB 驱动操作…"));
    } else if (!state().driver.known) {
        stateText = I18n::t(QStringLiteral("正在读取驱动状态…"));
    } else if (!hasTarget) {
        stateText = I18n::t(QStringLiteral("选择设备后可安装或恢复 WinUSB 驱动"));
    } else {
        const QString target = QStringLiteral("%1:%2")
                                   .arg(vid, 4, 16, QLatin1Char('0'))
                                   .arg(pid, 4, 16, QLatin1Char('0'));
        stateText = I18n::tf(QStringLiteral("目标设备：{target}"),
                             {{QStringLiteral("target"), target}});
    }
    m_driverState->setText(stateText);

    if (!state().driver.service.isEmpty()) {
        m_driverService->setText(I18n::tf(QStringLiteral("当前 Service：{service}"),
                                          {{QStringLiteral("service"), state().driver.service}}));
        m_driverService->show();
    } else if (state().hasDriverTarget) {
        m_driverService->setText(
            I18n::t(QStringLiteral("当前 Service：usbser.sys（需要替换）")));
        m_driverService->show();
    } else {
        m_driverService->hide();
    }

    if (state().driver.logPath.isEmpty()) {
        m_driverLog->hide();
        m_copyLog->setEnabled(false);
    } else {
        m_driverLog->setText(I18n::tf(QStringLiteral("驱动日志：{path}"),
                                      {{QStringLiteral("path"), state().driver.logPath}}));
        m_driverLog->show();
        m_copyLog->setEnabled(true);
    }

    m_installDriver->setEnabled(hasTarget && !state().driver.busy);
    m_restoreDriver->setEnabled(hasTarget && !state().driver.busy);
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
    refreshDriverCard();
    updateLastResult();
}
