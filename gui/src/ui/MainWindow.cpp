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
#include <QPainter>
#include <QProgressBar>
#include <QPushButton>
#include <QScrollArea>
#include <QSignalBlocker>
#include <QStackedWidget>
#include <QStyle>
#include <QTimer>
#include <QVBoxLayout>

#include "core/FfiApi.h"
#include "platform/Elevate.h"
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
#define AUROKU_APP_VERSION "0.1.0"
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
}

MainWindow::~MainWindow() = default;

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

#ifdef Q_OS_LINUX
    m_privilege = Ui::label(elevated() ? QStringLiteral("权限：root")
                                       : QStringLiteral("权限：普通用户 · USB 访问可能受限"),
                            QStringLiteral("subtle"), m_footer);
    layout->addWidget(m_privilege);
#endif

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

        m_state.selectedDevice = -1;
        quint16 vid = 0;
        quint16 pid = 0;
        if (parseHexU16(m_state.vidText, &vid, nullptr)
            && parseHexU16(m_state.pidText, &pid, nullptr)) {
            for (int i = 0; i < m_state.devices.size(); ++i) {
                if (m_state.devices.at(i).vid == vid && m_state.devices.at(i).pid == pid) {
                    selectDevice(i, false);
                    break;
                }
            }
        }

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

    const QString label = I18n::t(labelKey);
    m_state.busy = true;
    m_state.busyLabel = label;
    logLine(QStringLiteral("▶ ") + label);
    m_bridge.runAsync(request.request, label);
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

    m_state.scanning = true;
    m_state.scanError.clear();
    m_bridge.scanAsync(vid, pid);
    updateChrome();
    refreshPages();
}

void MainWindow::selectDevice(int index, bool announce)
{
    if (index < 0 || index >= m_state.devices.size())
        return;

    const DeviceInfo &device = m_state.devices.at(index);
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

bool MainWindow::elevated() const
{
    return Elevate::isEffectiveRoot();
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
          "跨平台：Linux、Windows 与 macOS。USB 访问在 Linux 上通常需要 udev 规则或 root 权限。",
          "Linux 启动时会通过 pkexec（polkit）请求 root 权限；使用 --no-polkit 或 AUROKU_GUI_NO_POLKIT=1 可跳过。认证取消或 polkit 不可用时会以普通用户继续运行。",
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
