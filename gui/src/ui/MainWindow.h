#pragma once

#include <QMainWindow>

#include "core/AppState.h"
#include "core/PageHost.h"
#include "core/RustBridge.h"
#include "platform/UsbAccess.h"

class QComboBox;
class QCloseEvent;
class QFrame;
class QLabel;
class QProgressBar;
class QPushButton;
class QStackedWidget;
class QTimer;
class ConsolePage;
class OverviewPage;
class Page;

/// Application shell: navigation rail, stacked pages, footer and toast.
class MainWindow : public QMainWindow, public PageHost {
    Q_OBJECT
public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow() override;

    // PageHost ---------------------------------------------------------------
    AppState &state() override { return m_state; }
    RustBridge &bridge() override { return m_bridge; }
    void startTask(const QJsonObject &task, const QString &labelKey) override;
    void scanDevices(bool filtered) override;
    void selectDevice(int index, bool announce) override;
    void refreshDriverStatus() override;
    bool installDriver(quint16 vid, quint16 pid, bool restoreOnExit) override;
    bool restoreDriver(quint16 vid, quint16 pid) override;
    void showToast(const QString &text, Tone tone) override;
    void confirmAction(const QString &titleKey, const QString &body, const QString &actionKey,
                       const std::function<void()> &action) override;
    void goToPage(PageId page) override;
protected:
    void closeEvent(QCloseEvent *event) override;
    void resizeEvent(QResizeEvent *event) override;

private:
    void buildUi();
    QWidget *buildNavRail();
    QWidget *buildFooter();
    void buildPages();
    void buildToast(QWidget *parent);
    void connectBridge();

    void logLine(const QString &text);
    void handleDriverFinished(const QString &op, bool ok, const QJsonObject &data,
                              const QString &kind, const QString &message);
    void applyDriverStatus(const QJsonObject &data);
    /// Restore a pending WinUSB driver before closing; returns false on failure.
    bool restoreDriverBeforeClose(QString *error);
    void updateChrome();
    void refreshPages();
    void retranslateUi();
    void positionToast();
    void showAbout();
    void toggleTheme();

    QPixmap logoPixmap() const;

    AppState m_state;
    RustBridge m_bridge;
    UsbAccess m_usbAccess;

    QStackedWidget *m_stack = nullptr;
    QVector<Page *> m_pages;
    OverviewPage *m_overview = nullptr;
    ConsolePage *m_console = nullptr;

    QFrame *m_navRail = nullptr;
    QVector<QPushButton *> m_navButtons;
    QPushButton *m_themeButton = nullptr;
    QComboBox *m_languageCombo = nullptr;
    QLabel *m_logo = nullptr;

    QFrame *m_footer = nullptr;
    QLabel *m_statusChip = nullptr;
    QProgressBar *m_spinner = nullptr;
    QLabel *m_busyText = nullptr;
    QLabel *m_versionLabel = nullptr;

    QFrame *m_toast = nullptr;
    QLabel *m_toastIcon = nullptr;
    QLabel *m_toastText = nullptr;
    QTimer *m_toastTimer = nullptr;

    bool m_updatingLanguage = false;
    int m_driverStatusRetries = 0;
};
