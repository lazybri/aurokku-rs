#include "MainWindow.h"

#include <QButtonGroup>
#include <QCloseEvent>
#include <QComboBox>
#include <QDialog>
#include <QFont>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLabel>
#include <QMessageBox>
#include <QPainter>
#include <QProgressBar>
#include <QPushButton>
#include <QDebug>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QStackedWidget>
#include <QStyle>
#include <QTimer>
#include <QVBoxLayout>

#include "core/FfiApi.h"
#include "ui/ConfirmDialog.h"
#include "ui/Fonts.h"
#include "ui/I18n.h"
#include "ui/Icons.h"
#include "ui/Page.h"
#include "ui/Theme.h"
#include "ui/Widgets.h"
#include "ui/pages/BatchPage.h"
#include "ui/pages/ConsolePage.h"
#include "ui/pages/OverviewPage.h"
#include "ui/pages/PinPage.h"
#include "ui/pages/ProbePage.h"
#include "ui/pages/SessionPage.h"
#include "ui/pages/UimPage.h"

#ifndef AUROKU_APP_VERSION
#define AUROKU_APP_VERSION "0.3.2-qt6"
#endif

namespace {

QColor toneColor(Tone tone)
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

Icon toneIcon(Tone tone)
{
    switch (tone) {
    case Tone::Success:
        return Icon::Check;
    case Tone::Error:
    case Tone::Warning:
        return Icon::Warning;
    case Tone::Primary:
    case Tone::Tertiary:
        return Icon::Info;
    case Tone::Neutral:
        break;
    }
    return Icon::Info;
}

} // namespace

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
{
    setWindowTitle(QStringLiteral("auRokku"));
    resize(1220, 800);
    setMinimumSize(1000, 660);

    buildUi();
    connectBridge();

    m_overview->syncFormFromState();
    logLine(I18n::t(QStringLiteral("界面已启动，正在扫描 USB 总线")));
    updateChrome();

    QTimer::singleShot(160, this, [this] { scanDevices(false); });
    // Explicit driver probe: surfaces a pending_restore left by a crashed run
    // and feeds the WinUSB driver card on the Overview page.
    QTimer::singleShot(900, this, [this] { refreshDriverStatus(); });
}

MainWindow::~MainWindow()
{
    QString error;
    if (!m_usbAccess.revoke(&error) && !error.isEmpty())
        qWarning().noquote() << "Failed to restore USB ACL:" << error;
}

void MainWindow::closeEvent(QCloseEvent *event)
{
    qInfo().noquote() << "closeEvent: driver busy =" << m_state.driver.busy
                      << "restore-on-exit =" << m_state.restoreDriverOnExit;
    if (m_state.driver.busy) {
        QMessageBox::warning(
            this, I18n::t(QStringLiteral("驱动操作进行中")),
            I18n::t(QStringLiteral("正在安装或恢复 WinUSB 驱动，请等待操作完成后再关闭。")));
        event->ignore();
        return;
    }

    QString aclError;
    if (!m_usbAccess.revoke(&aclError)) {
        QMessageBox::warning(this, I18n::t(QStringLiteral("USB ACL 清理失败")),
                             I18n::tf(QStringLiteral("USB ACL 清理失败：{error}"),
                                      {{QStringLiteral("error"), aclError}}));
        event->ignore();
        return;
    }

    // Opt-in: restore the original driver before the process exits. This is a
    // blocking FFI call (one UAC prompt at most), matching the synchronous ACL
    // cleanup above; a failure blocks the close instead of hiding a WinUSB
    // driver left behind on the device.
    if (m_state.restoreDriverOnExit) {
        QString driverError;
        qInfo().noquote() << "closeEvent: restoring the pending WinUSB driver";
        if (!restoreDriverBeforeClose(&driverError)) {
            qWarning().noquote() << "closeEvent: driver restore failed:" << driverError;
            const auto choice = QMessageBox::question(
                this, I18n::t(QStringLiteral("恢复原始驱动失败")),
                I18n::tf(QStringLiteral("恢复原始驱动失败：{error}"),
                         {{QStringLiteral("error"), driverError}})
                    + QStringLiteral("\n\n") + I18n::t(QStringLiteral("是否仍然退出？")),
                QMessageBox::Yes | QMessageBox::No, QMessageBox::No);
            if (choice == QMessageBox::Yes) {
                event->accept();
                return;
            }
            event->ignore();
            return;
        }
    }

    event->accept();
}

// ==============================================================================
// UI construction
// ==============================================================================

void MainWindow::buildUi()
{
    auto *central = new QWidget(this);
    central->setObjectName(QStringLiteral("appRoot"));

    auto *root = new QHBoxLayout(central);
    root->setContentsMargins(0, 0, 0, 0);
    root->setSpacing(0);

    m_stack = new QStackedWidget(central);
    buildPages();

    root->addWidget(buildNavRail());

    auto *right = new QWidget(central);
    auto *rightLayout = new QVBoxLayout(right);
    rightLayout->setContentsMargins(0, 0, 0, 0);
    rightLayout->setSpacing(0);
    rightLayout->addWidget(m_stack, 1);
    rightLayout->addWidget(buildFooter());
    root->addWidget(right, 1);

    setCentralWidget(central);
    buildToast(central);
    goToPage(PageId::Overview);
}

void MainWindow::buildPages()
{
    const auto addPage = [this](Page *page) {
        auto *scroll = new QScrollArea(m_stack);
        scroll->setWidgetResizable(true);
        scroll->setFrameShape(QFrame::NoFrame);
        scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        scroll->viewport()->setAutoFillBackground(false);
        scroll->setWidget(page);
        m_stack->addWidget(scroll);
        m_pages.append(page);
    };

    auto *overview = new OverviewPage(this);
    addPage(overview);
    m_overview = overview;

    addPage(new UimPage(this));
    addPage(new PinPage(this));
    addPage(new ProbePage(this));
    addPage(new BatchPage(this));
    addPage(new SessionPage(this));

    auto *console = new ConsolePage(this);
    addPage(console);
    m_console = console;
}

QWidget *MainWindow::buildNavRail()
{
    m_navRail = new QFrame;
    m_navRail->setObjectName(QStringLiteral("navRail"));
    m_navRail->setFixedWidth(240);

    auto *layout = new QVBoxLayout(m_navRail);
    layout->setContentsMargins(12, 16, 12, 12);
    layout->setSpacing(4);

    auto *brand = new QWidget(m_navRail);
    auto *brandLayout = new QHBoxLayout(brand);
    brandLayout->setContentsMargins(4, 0, 0, 0);
    brandLayout->setSpacing(10);

    m_logo = new QLabel(brand);
    m_logo->setFixedSize(42, 42);
    m_logo->setPixmap(logoPixmap());
    brandLayout->addWidget(m_logo);

    auto *titles = new QVBoxLayout;
    titles->setContentsMargins(0, 0, 0, 0);
    titles->setSpacing(1);
    auto *name = new QLabel(QStringLiteral("auRokku"), brand);
    name->setObjectName(QStringLiteral("brandTitle"));
    titles->addWidget(name);
    titles->addWidget(Ui::label(QStringLiteral("SERI / Type2Auth 设备工具"),
                                QStringLiteral("brandSubtitle"), brand));
    brandLayout->addLayout(titles, 1);
    layout->addWidget(brand);
    layout->addSpacing(12);

    struct NavItem {
        PageId id;
        Icon icon;
        const char *label;
    };
    static const NavItem items[] = {
        {PageId::Overview, Icon::Dashboard, "概览"},
        {PageId::Uim, Icon::Lock, "UIM 锁"},
        {PageId::Pin, Icon::Key, "PIN / ICCID"},
        {PageId::Probe, Icon::Probe, "探测"},
        {PageId::Batch, Icon::List, "批量"},
        {PageId::Session, Icon::Power, "会话"},
        {PageId::Console, Icon::Terminal, "控制台"},
    };

    auto *group = new QButtonGroup(m_navRail);
    group->setExclusive(true);
    const QColor navIconColor = Theme::instance().palette().onSurfaceVariant;

    for (const NavItem &item : items) {
        auto *button = new QPushButton(I18n::t(QString::fromUtf8(item.label)), m_navRail);
        button->setObjectName(QStringLiteral("navButton"));
        button->setProperty("i18nKey", QString::fromUtf8(item.label));
        button->setCheckable(true);
        button->setCursor(Qt::PointingHandCursor);
        button->setIcon(Icons::icon(item.icon, navIconColor, 18));
        button->setIconSize(QSize(18, 18));
        button->setProperty("iconId", static_cast<int>(item.icon));
        button->setProperty("iconRole", QStringLiteral("onSurfaceVariant"));
        button->setProperty("iconSize", 18);
        button->setMinimumHeight(32);
        button->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        group->addButton(button, static_cast<int>(item.id));
        layout->addWidget(button);
        m_navButtons.append(button);

        connect(button, &QPushButton::clicked, this,
                [this, id = item.id] { goToPage(id); });
    }

    layout->addStretch(1);
    layout->addWidget(Ui::separator(m_navRail));
    layout->addSpacing(6);

    m_themeButton = Ui::button(QStringLiteral("主题"), Ui::ButtonVariant::Text,
                               Theme::instance().dark() ? Icon::Sun : Icon::Moon, m_navRail);
    m_themeButton->setToolTip(I18n::t(QStringLiteral("切换浅色 / 深色主题")));
    m_themeButton->setProperty("i18nTip", QStringLiteral("切换浅色 / 深色主题"));
    m_themeButton->setMinimumHeight(30);
    layout->addWidget(m_themeButton);
    connect(m_themeButton, &QPushButton::clicked, this, [this] { toggleTheme(); });

    auto *languageLabel = Ui::label(QStringLiteral("界面语言"), QStringLiteral("caption"), m_navRail);
    languageLabel->setContentsMargins(4, 6, 0, 0);
    layout->addWidget(languageLabel);

    m_languageCombo = new QComboBox(m_navRail);
    m_languageCombo->addItem(QStringLiteral("中文"));
    m_languageCombo->addItem(QStringLiteral("English"));
    m_languageCombo->setCurrentIndex(I18n::instance().language() == I18n::Language::ZhCn ? 0 : 1);
    layout->addWidget(m_languageCombo);

    auto *about = Ui::button(QStringLiteral("关于"), Ui::ButtonVariant::Text, Icon::About,
                             m_navRail);
    about->setMinimumHeight(30);
    layout->addWidget(about);
    connect(about, &QPushButton::clicked, this, [this] { showAbout(); });

    connect(m_languageCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this,
            [this](int index) {
                if (m_updatingLanguage)
                    return;
                I18n::instance().setLanguage(index == 0 ? I18n::Language::ZhCn
                                                        : I18n::Language::En);
                retranslateUi();
                showToast(I18n::tf(QStringLiteral("界面语言已切换为 {lang}"),
                                   {{QStringLiteral("lang"), I18n::instance().nativeName()}}),
                          Tone::Primary);
            });

    return m_navRail;
}

QWidget *MainWindow::buildFooter()
{
    m_footer = new QFrame;
    m_footer->setObjectName(QStringLiteral("footer"));
    m_footer->setFixedHeight(48);

    auto *layout = new QHBoxLayout(m_footer);
    layout->setContentsMargins(24, 0, 24, 0);
    layout->setSpacing(12);

    m_statusChip = Ui::chip(I18n::t(QStringLiteral("就绪")), Tone::Neutral, m_footer);
    layout->addWidget(m_statusChip);

    m_spinner = new QProgressBar(m_footer);
    m_spinner->setObjectName(QStringLiteral("busySpinner"));
    m_spinner->setRange(0, 0);
    m_spinner->setTextVisible(false);
    m_spinner->setFixedSize(110, 12);
    m_spinner->hide();
    layout->addWidget(m_spinner);

    m_busyText = Ui::label(QStringLiteral("等待操作"), QStringLiteral("subtle"), m_footer);
    layout->addWidget(m_busyText);
    layout->addStretch(1);

    m_versionLabel = new QLabel(
        QStringLiteral("auRokku v%1 · Qt %2").arg(QStringLiteral(AUROKU_APP_VERSION), qVersion()),
        m_footer);
    m_versionLabel->setObjectName(QStringLiteral("subtle"));
    layout->addWidget(m_versionLabel);

    return m_footer;
}

void MainWindow::buildToast(QWidget *parent)
{
    m_toast = new QFrame(parent);
    m_toast->setObjectName(QStringLiteral("toast"));
    m_toast->setAttribute(Qt::WA_TransparentForMouseEvents);

    auto *layout = new QHBoxLayout(m_toast);
    layout->setContentsMargins(12, 8, 14, 8);
    layout->setSpacing(8);

    m_toastIcon = new QLabel(m_toast);
    m_toastIcon->setFixedSize(18, 18);
    layout->addWidget(m_toastIcon);

    m_toastText = new QLabel(m_toast);
    m_toastText->setObjectName(QStringLiteral("toastText"));
    m_toastText->setWordWrap(true);
    m_toastText->setMaximumWidth(460);
    layout->addWidget(m_toastText);

    m_toast->hide();

    m_toastTimer = new QTimer(this);
    m_toastTimer->setSingleShot(true);
    m_toastTimer->setInterval(3800);
    connect(m_toastTimer, &QTimer::timeout, m_toast, &QWidget::hide);
}

void MainWindow::connectBridge()
{
    connect(&m_bridge, &RustBridge::scanStarted, this, [this] {
        m_state.scanning = true;
        m_state.scanError.clear();
        logLine(I18n::t(QStringLiteral("开始扫描 USB 设备")));
        updateChrome();
        refreshPages();
    });

    connect(&m_bridge, &RustBridge::scanFinished, this, [this](const QJsonArray &devices) {
        m_state.scanning = false;
        m_state.devices.clear();
        m_state.devices.reserve(devices.size());
        for (const QJsonValue &value : devices)
            m_state.devices.append(DeviceInfo::fromJson(value));

        logLine(I18n::tf(QStringLiteral("扫描完成，发现 {n} 个 USB 设备"),
                         {{QStringLiteral("n"), m_state.devices.size()}}));

        // First scan finished: ask the backend for the WinUSB driver state.
        if (!m_state.driver.known)
            refreshDriverStatus();

        updateChrome();
        refreshPages();
    });

    connect(&m_bridge, &RustBridge::scanFailed, this, [this](const QString &message) {
        m_state.scanning = false;
        m_state.scanError = message;
        logLine(I18n::tf(QStringLiteral("扫描失败：{message}"),
                         {{QStringLiteral("message"), message}}));
        showToast(I18n::tf(QStringLiteral("扫描失败：{message}"),
                           {{QStringLiteral("message"), message}}),
                  Tone::Error);
        updateChrome();
        refreshPages();
    });

    connect(&m_bridge, &RustBridge::taskStarted, this, [this](const QString &label) {
        m_state.busy = true;
        m_state.busyLabel = label;
        updateChrome();
        refreshPages();
    });

    connect(&m_bridge, &RustBridge::logLine, this, [this](const QString &line) {
        logLine(line);
    });

    connect(&m_bridge, &RustBridge::taskRejected, this, [this](const QString &reason) {
        if (reason.isEmpty() || reason == QLatin1String("busy"))
            showToast(I18n::t(QStringLiteral("当前操作不可用")), Tone::Warning);
        else
            showToast(reason, Tone::Warning);
    });

    connect(&m_bridge, &RustBridge::driverFinished, this, &MainWindow::handleDriverFinished);

    connect(&m_bridge, &RustBridge::taskFinished, this,
            [this](const QString &label, bool ok, const QJsonObject &data, const QString &kind,
                   const QString &message) {
                m_state.busy = false;
                m_state.busyLabel.clear();

                if (ok) {
                    const bool success = data.value(QStringLiteral("success")).toBool(false);
                    m_state.lastSuccess = success;
                    m_state.lastOp = data.value(QStringLiteral("operation")).toString(label);
                    m_state.result = data;
                    m_state.hasResult = true;
                    m_state.prettyJson = QString::fromUtf8(
                        QJsonDocument(data).toJson(QJsonDocument::Indented));
                    m_state.lastErrorKind.clear();
                    m_state.lastErrorMessage.clear();

                    const QString status =
                        I18n::t(success ? QStringLiteral("成功") : QStringLiteral("结果为失败"));
                    logLine(I18n::tf(QStringLiteral("{label} 完成：{status}"),
                                     {{QStringLiteral("label"), label},
                                      {QStringLiteral("status"), status}}));
                    showToast(success
                                  ? I18n::tf(QStringLiteral("{label}：成功"),
                                             {{QStringLiteral("label"), label}})
                                  : I18n::tf(QStringLiteral("{label}：结果为失败"),
                                             {{QStringLiteral("label"), label}}),
                              success ? Tone::Success : Tone::Warning);
                } else {
                    m_state.lastSuccess = false;
                    m_state.lastOp = label;
                    m_state.hasResult = false;
                    m_state.result = QJsonObject();
                    m_state.prettyJson.clear();
                    m_state.lastErrorKind = kind;
                    m_state.lastErrorMessage = message;

                    logLine(I18n::tf(QStringLiteral("{label} 错误（{kind}）：{message}"),
                                     {{QStringLiteral("label"), label},
                                      {QStringLiteral("kind"), kind},
                                      {QStringLiteral("message"), message}}));
                    showToast(I18n::tf(QStringLiteral("{kind}：{message}"),
                                       {{QStringLiteral("kind"), kind},
                                        {QStringLiteral("message"), message}}),
                              Tone::Error);

                    if (kind == QLatin1String("DriverError")) {
                        // Remember the device and the task so a successful driver
                        // install can retry it automatically.
                        const QJsonObject request = m_state.lastTaskRequest;
                        m_state.hasDriverTarget = true;
                        m_state.driverTargetVid = static_cast<quint16>(
                            request.value(QStringLiteral("vid")).toInt());
                        m_state.driverTargetPid = static_cast<quint16>(
                            request.value(QStringLiteral("pid")).toInt());
                        m_state.hasRetryTask = !request.isEmpty();
                        logLine(I18n::t(QStringLiteral(
                            "该设备需要 WinUSB 驱动；可在概览页的 WinUSB 驱动卡片中安装。")));
                        goToPage(PageId::Overview);
                        refreshDriverStatus();
                    }
                }

                updateChrome();
                refreshPages();
            });
}

// ==============================================================================
// PageHost
// ==============================================================================

void MainWindow::startTask(const QJsonObject &task, const QString &labelKey)
{
    if (m_bridge.busy() || m_state.busy || m_state.scanning) {
        showToast(I18n::t(QStringLiteral("当前操作不可用")), Tone::Warning);
        return;
    }

    const RequestResult request = buildRequest(m_state, task);
    if (!request.ok) {
        showToast(request.error, Tone::Error);
        return;
    }

    QJsonObject taskRequest = request.request;
#if defined(Q_OS_LINUX) || defined(__linux__)
    if (m_state.selectedDevice < 0 || m_state.selectedDevice >= m_state.devices.size()) {
        showToast(I18n::t(QStringLiteral("请先扫描并选择 USB 设备")), Tone::Warning);
        return;
    }
    const DeviceInfo &device = m_state.devices.at(m_state.selectedDevice);
    if (!m_usbAccess.isGrantedFor(device.bus, device.address)) {
        showToast(I18n::t(QStringLiteral("所选 USB 设备的临时访问权限已失效，请重新选择设备")),
                  Tone::Error);
        return;
    }
    if (request.request.value(QStringLiteral("vid")).toInt() != device.vid
        || request.request.value(QStringLiteral("pid")).toInt() != device.pid) {
        showToast(I18n::t(QStringLiteral("VID / PID 与所选 USB 设备不匹配")), Tone::Error);
        return;
    }
    taskRequest.insert(QStringLiteral("bus"), device.bus);
    taskRequest.insert(QStringLiteral("address"), device.address);
#endif

    const QString label = I18n::t(labelKey);
    m_state.busy = true;
    m_state.busyLabel = label;
    m_state.lastTaskRequest = taskRequest;
    logLine(QStringLiteral("▶ ") + label);
    if (!m_bridge.runAsync(taskRequest, label)) {
        m_state.busy = false;
        m_state.busyLabel.clear();
        updateChrome();
        refreshPages();
        return;
    }
    updateChrome();
    refreshPages();
}

void MainWindow::scanDevices(bool filtered)
{
    if (m_state.busy || m_state.scanning)
        return;

    int vid = -1;
    int pid = -1;
    if (filtered) {
        quint16 parsedVid = 0;
        quint16 parsedPid = 0;
        if (!parseHexU16(m_state.vidText, &parsedVid, nullptr)
            || !parseHexU16(m_state.pidText, &parsedPid, nullptr)) {
            showToast(I18n::t(QStringLiteral("VID / PID 格式无效")), Tone::Error);
            return;
        }
        vid = parsedVid;
        pid = parsedPid;
    }

    QString aclError;
    if (!m_usbAccess.revoke(&aclError)) {
        showToast(I18n::tf(QStringLiteral("USB ACL 清理失败：{error}"),
                           {{QStringLiteral("error"), aclError}}),
                  Tone::Error);
        refreshPages();
        return;
    }

    m_state.selectedDevice = -1;
    m_state.devices.clear();
    m_state.scanning = true;
    m_state.scanError.clear();
    if (!m_bridge.scanAsync(vid, pid)) {
        m_state.scanning = false;
        updateChrome();
        refreshPages();
        return;
    }
    updateChrome();
    refreshPages();
}

void MainWindow::selectDevice(int index, bool announce)
{
    if (index < -1 || index >= m_state.devices.size())
        return;

    if (index == m_state.selectedDevice)
        return;

    QString aclError;
    if (!m_usbAccess.revoke(&aclError)) {
        showToast(I18n::tf(QStringLiteral("USB ACL 清理失败：{error}"),
                           {{QStringLiteral("error"), aclError}}),
                  Tone::Error);
        refreshPages();
        return;
    }

    m_state.selectedDevice = -1;
    if (index == -1) {
        refreshPages();
        return;
    }

    const DeviceInfo &device = m_state.devices.at(index);
#if defined(Q_OS_LINUX) || defined(__linux__)
    if (!m_usbAccess.grant(device.bus, device.address, &aclError)) {
        logLine(I18n::tf(QStringLiteral("无法授予所选 USB 设备访问权限：{error}"),
                         {{QStringLiteral("error"), aclError}}));
        showToast(I18n::tf(QStringLiteral("无法授予所选 USB 设备访问权限：{error}"),
                           {{QStringLiteral("error"), aclError}}),
                  Tone::Error);
        refreshPages();
        return;
    }
#endif

    m_state.selectedDevice = index;
    m_state.vidText = QStringLiteral("%1").arg(device.vid, 4, 16, QLatin1Char('0'));
    m_state.pidText = QStringLiteral("%1").arg(device.pid, 4, 16, QLatin1Char('0'));
    if (!device.serial.isEmpty())
        m_state.snText = device.serial;

    if (m_overview != nullptr)
        m_overview->syncFormFromState();

    if (announce) {
        const QString message =
            I18n::tf(QStringLiteral("已选择 {name}（{id}）"),
                     {{QStringLiteral("name"), device.display}, {QStringLiteral("id"), device.id}});
        logLine(message);
        showToast(message, Tone::Primary);
    }
}

// ==============================================================================
// WinUSB driver (WinUSB driver card page hooks)
// ==============================================================================

void MainWindow::refreshDriverStatus()
{
    if (m_bridge.busy()) {
        // The worker is owned by a scan/task/driver operation; retry shortly.
        // Status polling is internal, so it must never toast a rejection.
        if (m_driverStatusRetries < 8) {
            ++m_driverStatusRetries;
            QTimer::singleShot(1200, this, [this] { refreshDriverStatus(); });
        }
        return;
    }
    m_driverStatusRetries = 0;
    m_bridge.driverStatusAsync();
}

bool MainWindow::installDriver(quint16 vid, quint16 pid, bool restoreOnExit)
{
    if (m_bridge.busy() || m_state.busy || m_state.scanning) {
        showToast(I18n::t(QStringLiteral("当前操作不可用")), Tone::Warning);
        return false;
    }

    m_state.restoreDriverOnExit = restoreOnExit;
    m_state.driver.busy = true;
    m_state.driver.busyOp = QStringLiteral("install");
    m_state.busy = true;
    m_state.busyLabel = I18n::t(QStringLiteral("正在安装 WinUSB 驱动…"));
    logLine(m_state.busyLabel);
    if (!m_state.driver.logPath.isEmpty())
        logLine(I18n::tf(QStringLiteral("驱动日志：{path}"),
                         {{QStringLiteral("path"), m_state.driver.logPath}}));

    if (!m_bridge.driverInstallAsync(static_cast<int>(vid), static_cast<int>(pid),
                                     restoreOnExit)) {
        m_state.driver.busy = false;
        m_state.driver.busyOp.clear();
        m_state.busy = false;
        m_state.busyLabel.clear();
        updateChrome();
        refreshPages();
        return false;
    }

    updateChrome();
    refreshPages();
    return true;
}

bool MainWindow::restoreDriver(quint16 vid, quint16 pid)
{
    if (m_bridge.busy() || m_state.busy || m_state.scanning) {
        showToast(I18n::t(QStringLiteral("当前操作不可用")), Tone::Warning);
        return false;
    }

    m_state.driver.busy = true;
    m_state.driver.busyOp = QStringLiteral("restore");
    m_state.busy = true;
    m_state.busyLabel = I18n::t(QStringLiteral("正在恢复原始驱动…"));
    logLine(m_state.busyLabel);
    if (!m_state.driver.logPath.isEmpty())
        logLine(I18n::tf(QStringLiteral("驱动日志：{path}"),
                         {{QStringLiteral("path"), m_state.driver.logPath}}));

    if (!m_bridge.driverRestoreAsync(static_cast<int>(vid), static_cast<int>(pid))) {
        m_state.driver.busy = false;
        m_state.driver.busyOp.clear();
        m_state.busy = false;
        m_state.busyLabel.clear();
        updateChrome();
        refreshPages();
        return false;
    }

    updateChrome();
    refreshPages();
    return true;
}

void MainWindow::handleDriverFinished(const QString &op, bool ok, const QJsonObject &data,
                                      const QString &kind, const QString &message)
{
    if (op == QLatin1String("status")) {
        if (ok) {
            applyDriverStatus(data);
        } else {
            const QString detail = message.isEmpty() ? kind : kind + QStringLiteral(": ") + message;
            logLine(I18n::tf(QStringLiteral("无法读取驱动状态：{message}"),
                             {{QStringLiteral("message"), detail}}));
        }
        refreshPages();
        return;
    }

    m_state.driver.busy = false;
    m_state.driver.busyOp.clear();
    m_state.busy = false;
    m_state.busyLabel.clear();

    if (ok && op == QLatin1String("install")) {
        const quint16 vid = static_cast<quint16>(data.value(QStringLiteral("vid")).toInt());
        const quint16 pid = static_cast<quint16>(data.value(QStringLiteral("pid")).toInt());
        const QString target = QStringLiteral("%1:%2")
                                   .arg(vid, 4, 16, QLatin1Char('0'))
                                   .arg(pid, 4, 16, QLatin1Char('0'));
        logLine(I18n::tf(QStringLiteral("已为 {target} 安装 WinUSB 驱动"),
                         {{QStringLiteral("target"), target}}));
        showToast(I18n::tf(QStringLiteral("已为 {target} 安装 WinUSB 驱动"),
                           {{QStringLiteral("target"), target}}),
                  Tone::Success);

        m_state.hasDriverTarget = false;
        if (m_state.hasRetryTask && !m_state.lastTaskRequest.isEmpty()) {
            const QJsonObject request = m_state.lastTaskRequest;
            const QString label = m_state.lastTaskLabel;
            m_state.hasRetryTask = false;
            logLine(I18n::t(QStringLiteral("WinUSB 驱动已安装，正在重试…")));
            if (m_bridge.runAsync(request, label)) {
                m_state.busy = true;
                m_state.busyLabel = label;
            } else {
                m_state.hasRetryTask = true;
            }
        } else {
            refreshDriverStatus();
        }
    } else if (ok && op == QLatin1String("restore")) {
        logLine(I18n::t(QStringLiteral("已恢复原始 USB 驱动")));
        showToast(I18n::t(QStringLiteral("已恢复原始 USB 驱动")), Tone::Primary);
        m_state.hasDriverTarget = false;
        m_state.hasRetryTask = false;
        refreshDriverStatus();
    } else {
        const QString detail = message.isEmpty() ? kind : kind + QStringLiteral(": ") + message;
        logLine(I18n::tf(QStringLiteral("驱动操作失败：{message}"),
                         {{QStringLiteral("message"), detail}}));
        showToast(I18n::tf(QStringLiteral("驱动操作失败：{message}"),
                           {{QStringLiteral("message"), detail}}),
                  Tone::Error);
        refreshDriverStatus();
    }

    updateChrome();
    refreshPages();
}

void MainWindow::applyDriverStatus(const QJsonObject &data)
{
    DriverInfo &driver = m_state.driver;
    const bool firstLook = !driver.known;
    driver.known = true;
    driver.supported = data.value(QStringLiteral("supported")).toBool();
    driver.logPath = data.value(QStringLiteral("log_path")).toString();
    driver.stateDir = data.value(QStringLiteral("state_dir")).toString();
    driver.service = data.value(QStringLiteral("service")).toString();

    const QJsonValue pending = data.value(QStringLiteral("pending"));
    driver.pending = pending.isObject();
    if (driver.pending) {
        const QJsonObject record = pending.toObject();
        driver.pendingVid =
            static_cast<quint16>(record.value(QStringLiteral("vid")).toInt());
        driver.pendingPid =
            static_cast<quint16>(record.value(QStringLiteral("pid")).toInt());
        driver.pendingRestoreOnExit =
            record.value(QStringLiteral("restore_on_exit")).toBool();
        // Do not opt this session in automatically: the crash record only
        // drives the warning/card, the user decides again whether to restore.
    } else {
        driver.pendingVid = 0;
        driver.pendingPid = 0;
        driver.pendingRestoreOnExit = false;
    }

    if (firstLook && driver.pending) {
        const QString target = driver.pendingTarget();
        const QString message = I18n::tf(
            QStringLiteral("上次会话未恢复 {target} 的原始 USB 驱动，可在概览页的 WinUSB 驱动卡片中恢复"),
            {{QStringLiteral("target"), target}});
        logLine(message);
        showToast(message, Tone::Warning);
        goToPage(PageId::Overview);
    }
}

bool MainWindow::restoreDriverBeforeClose(QString *error)
{
    // Refresh synchronously: the pending record may have been created by an
    // install in this very session (restore_on_exit chosen).
    char *rawStatus = auroku_driver_status();
    if (rawStatus != nullptr) {
        const QByteArray bytes(rawStatus);
        auroku_string_free(rawStatus);
        const QJsonDocument document = QJsonDocument::fromJson(bytes);
        if (document.isObject()) {
            const QJsonObject root = document.object();
            if (root.value(QStringLiteral("ok")).toBool())
                applyDriverStatus(root.value(QStringLiteral("data")).toObject());
        }
    }

    if (!m_state.driver.pending)
        return true;

    const quint16 vid = m_state.driver.pendingVid;
    const quint16 pid = m_state.driver.pendingPid;
    const QString target = m_state.driver.pendingTarget();
    logLine(I18n::tf(QStringLiteral("正在恢复 {target} 的原始 USB 驱动…"),
                     {{QStringLiteral("target"), target}}));

    char *rawRestore = auroku_driver_restore(vid, pid, nullptr, nullptr);
    bool restored = false;
    QString message;
    if (rawRestore == nullptr) {
        message = QStringLiteral("backend returned a null pointer");
    } else {
        const QByteArray bytes(rawRestore);
        auroku_string_free(rawRestore);
        const QJsonDocument document = QJsonDocument::fromJson(bytes);
        const QJsonObject root = document.object();
        restored = root.value(QStringLiteral("ok")).toBool();
        if (!restored) {
            message = root.value(QStringLiteral("message")).toString();
            if (message.isEmpty())
                message = root.value(QStringLiteral("kind")).toString();
            if (message.isEmpty())
                message = QStringLiteral("unknown error");
        }
    }

    if (restored) {
        m_state.driver.pending = false;
        logLine(I18n::t(QStringLiteral("已恢复原始 USB 驱动")));
        return true;
    }

    if (error != nullptr)
        *error = message;
    return false;
}

void MainWindow::showToast(const QString &text, Tone tone)
{
    if (m_toast == nullptr)
        return;

    m_toastText->setText(text);
    const QColor color = toneColor(tone);
    m_toastIcon->setPixmap(Icons::pixmap(toneIcon(tone), color, 18));

    m_toast->adjustSize();
    m_toast->show();
    positionToast();
    m_toast->raise();
    m_toastTimer->start();
}

void MainWindow::confirmAction(const QString &titleKey, const QString &body,
                               const QString &actionKey, const std::function<void()> &action)
{
    if (Dialogs::confirm(this, titleKey, body, actionKey) && action)
        action();
}

void MainWindow::goToPage(PageId page)
{
    const int index = static_cast<int>(page);
    if (index < 0 || index >= m_navButtons.size())
        return;
    m_navButtons.at(index)->setChecked(true);
    m_stack->setCurrentIndex(index);
}

// ==============================================================================
// Chrome
// ==============================================================================

void MainWindow::logLine(const QString &text)
{
    m_state.appendLog(text);
    if (m_console != nullptr)
        m_console->appendLine(m_state.logs.last());
}

void MainWindow::updateChrome()
{
    if (m_statusChip == nullptr)
        return;

    QString status = QStringLiteral("就绪");
    Tone tone = Tone::Neutral;
    if (m_state.busy) {
        status = QStringLiteral("执行中");
        tone = Tone::Primary;
    } else if (m_state.scanning) {
        status = QStringLiteral("扫描中");
    }

    m_statusChip->setText(I18n::t(status));
    m_statusChip->setProperty("tone", QString::fromLatin1(Ui::toneName(tone)));
    m_statusChip->style()->unpolish(m_statusChip);
    m_statusChip->style()->polish(m_statusChip);

    const bool working = m_state.busy || m_state.scanning;
    m_spinner->setVisible(working);

    if (m_state.busy) {
        m_busyText->setText(I18n::tf(QStringLiteral("正在执行：{label}"),
                                     {{QStringLiteral("label"), m_state.busyLabel}}));
    } else if (m_state.scanning) {
        m_busyText->setText(I18n::t(QStringLiteral("开始扫描 USB 设备")));
    } else {
        m_busyText->setText(I18n::t(QStringLiteral("等待操作")));
    }

    for (Page *page : m_pages)
        page->setBusy(working);
}

void MainWindow::refreshPages()
{
    for (Page *page : m_pages)
        page->refresh();
}

void MainWindow::retranslateUi()
{
    if (m_navRail != nullptr)
        Ui::retranslateTree(m_navRail);
    if (m_footer != nullptr)
        Ui::retranslateTree(m_footer);

    for (Page *page : m_pages)
        page->retranslate();

    if (m_logo != nullptr)
        m_logo->setPixmap(logoPixmap());

    if (m_themeButton != nullptr) {
        const Icon themeIcon = Theme::instance().dark() ? Icon::Sun : Icon::Moon;
        m_themeButton->setIcon(
            Icons::icon(themeIcon, Theme::instance().palette().primary, 18));
        m_themeButton->setProperty("iconId", static_cast<int>(themeIcon));
        m_themeButton->setProperty("iconRole", QStringLiteral("primary"));
        m_themeButton->setProperty("iconSize", 18);
    }

    {
        const QSignalBlocker blocker(m_languageCombo);
        m_languageCombo->setCurrentIndex(I18n::instance().language() == I18n::Language::ZhCn ? 0 : 1);
    }

    updateChrome();
    refreshPages();

    for (QWidget *widget : findChildren<QWidget *>())
        widget->update();
}

void MainWindow::toggleTheme()
{
    Theme::instance().setDark(!Theme::instance().dark());
    retranslateUi();
}

void MainWindow::positionToast()
{
    if (m_toast == nullptr || m_toast->isHidden())
        return;
    QWidget *central = centralWidget();
    if (central == nullptr)
        return;

    const int margin = 24;
    const int footerHeight = m_footer != nullptr ? m_footer->height() : 0;
    const int x = central->width() - m_toast->width() - margin;
    const int y = central->height() - m_toast->height() - footerHeight - 14;
    m_toast->move(qMax(margin, x), qMax(margin, y));
}

void MainWindow::resizeEvent(QResizeEvent *event)
{
    QMainWindow::resizeEvent(event);
    positionToast();
}

QPixmap MainWindow::logoPixmap() const
{
    const Palette &palette = Theme::instance().palette();
    const int size = 42;
    QPixmap pixmap(size * 2, size * 2);
    pixmap.setDevicePixelRatio(2.0);
    pixmap.fill(Qt::transparent);

    QPainter painter(&pixmap);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setPen(Qt::NoPen);
    painter.setBrush(palette.primary);
    painter.drawRoundedRect(QRectF(0.5, 0.5, size - 1.0, size - 1.0), 4, 4);

    QFont font = painter.font();
    font.setPixelSize(16);
    font.setBold(true);
    painter.setFont(font);
    painter.setPen(palette.onPrimary);
    painter.drawText(QRectF(0, 0, size, size), Qt::AlignCenter, QStringLiteral("au"));
    return pixmap;
}

void MainWindow::showAbout()
{
    QDialog dialog(this);
    dialog.setWindowTitle(I18n::t(QStringLiteral("关于 auRokku")));
    dialog.setMinimumWidth(500);

    auto *layout = new QVBoxLayout(&dialog);
    layout->setContentsMargins(18, 16, 18, 14);
    layout->setSpacing(8);

    auto *title = new QLabel(QStringLiteral("auRokku"), &dialog);
    title->setObjectName(QStringLiteral("pageTitle"));
    layout->addWidget(title);

    layout->addWidget(Ui::label(
        QStringLiteral("auRokku v{version} · 基于Yuu所提供的代码二次开发 ©lazybri"),
        QStringLiteral("subtle"), &dialog));

    layout->addWidget(Ui::separator(&dialog));

    for (const char *key :
         {"基于 Qt 6 Widgets 的前端，通过 C ABI 直接调用 Rust 后端。协议、Type2Auth 算法与 JSON 结构均与 auRokku.py 保持一致。",
          "跨平台：Linux、Windows 与 macOS。Linux 通过 ACL 为所选 USB 设备临时授予读写权限。",
          "Linux GUI 始终以普通用户运行；仅在修改所选设备 ACL 时通过 polkit 调用 setfacl。",
          "所有 USB 操作均在后台线程执行。"}) {
        auto *text = new QLabel(I18n::t(QString::fromUtf8(key)), &dialog);
        text->setWordWrap(true);
        text->setObjectName(QStringLiteral("subtle"));
        layout->addWidget(text);
    }

    const QString version = QStringLiteral("auRokku %1 · Qt %2")
                                .arg(QStringLiteral(AUROKU_APP_VERSION), qVersion());
    auto *versions = new QLabel(version, &dialog);
    versions->setObjectName(QStringLiteral("monoValue"));
    layout->addWidget(versions);

    char *ffiVersion = auroku_ffi_version();
    if (ffiVersion != nullptr) {
        auto *backend = new QLabel(
            QStringLiteral("%1: %2").arg(I18n::t(QStringLiteral("后端版本")),
                                         QString::fromUtf8(ffiVersion)),
            &dialog);
        backend->setObjectName(QStringLiteral("monoValue"));
        layout->addWidget(backend);
        auroku_string_free(ffiVersion);
    }

    auto *buttons = new QHBoxLayout;
    buttons->addStretch(1);
    auto *close = Ui::button(QStringLiteral("关闭"), Ui::ButtonVariant::Filled, Icon::Check, &dialog);
    buttons->addWidget(close);
    layout->addLayout(buttons);
    connect(close, &QPushButton::clicked, &dialog, &QDialog::accept);

    dialog.exec();
}
