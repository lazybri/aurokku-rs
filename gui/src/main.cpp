//! auRokku Qt front-end entry point.

#include <QApplication>
#include <QByteArray>
#include <QCoreApplication>
#include <QDateTime>
#include <QDebug>
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMutex>
#include <QStandardPaths>
#include <QStyleFactory>
#include <QSysInfo>
#include <QTimer>
#include <cstring>

#include "core/FfiApi.h"
#include "ui/Fonts.h"
#include "ui/I18n.h"
#include "ui/MainWindow.h"
#include "ui/Theme.h"

#ifndef AUROKU_APP_VERSION
#define AUROKU_APP_VERSION "0.3.2-qt6"
#endif

namespace {

/// Owns <state_dir>/gui.log. A release build has no console, so every
/// qDebug/qWarning/qCritical line is mirrored there (truncated on start-up).
QFile *g_guiLog = nullptr;

QString stateDirFromBackend()
{
    char *raw = auroku_state_dir();
    if (raw == nullptr)
        return {};
    const QByteArray bytes(raw);
    auroku_string_free(raw);
    const QJsonObject root = QJsonDocument::fromJson(bytes).object();
    if (!root.value(QStringLiteral("ok")).toBool())
        return {};
    return root.value(QStringLiteral("data")).toString();
}

void guiLogHandler(QtMsgType type, const QMessageLogContext &, const QString &message)
{
    static QMutex mutex;
    const QMutexLocker locker(&mutex);
    if (g_guiLog == nullptr || !g_guiLog->isOpen())
        return;

    const char *level = "info";
    switch (type) {
    case QtDebugMsg:
        level = "debug";
        break;
    case QtInfoMsg:
        level = "info";
        break;
    case QtWarningMsg:
        level = "warn";
        break;
    case QtCriticalMsg:
        level = "error";
        break;
    case QtFatalMsg:
        level = "fatal";
        break;
    }

    const QString line = QStringLiteral("%1 [%2] %3\n")
                             .arg(QDateTime::currentDateTime().toString(
                                 QStringLiteral("yyyy-MM-dd HH:mm:ss.zzz")),
                                  QString::fromLatin1(level), message);
    g_guiLog->write(line.toUtf8());
    g_guiLog->flush();
}

/// Start writing gui.log before the window exists, so start-up failures are
/// captured too. The directory comes from the Rust backend (same as
/// driver.log), so both logs live next to the driver state.
void installGuiLog()
{
    const QString dir = stateDirFromBackend();
    if (dir.isEmpty())
        return;
    QDir().mkpath(dir);
    auto *file = new QFile(QDir(dir).filePath(QStringLiteral("gui.log")));
    if (!file->open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        delete file;
        return;
    }
    g_guiLog = file;
    qInstallMessageHandler(guiLogHandler);
}

} // namespace

int main(int argc, char **argv)
{
    bool smokeTest = false;
    bool darkTheme = true;
    int pageIndex = -1;
    QString screenshotPath;
    QString languageOverride;
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--smoke-test") == 0)
            smokeTest = true;
        else if (std::strcmp(argv[i], "--dark") == 0)
            darkTheme = true;
        else if (std::strcmp(argv[i], "--light") == 0)
            darkTheme = false;
        else if (std::strncmp(argv[i], "--page=", 7) == 0)
            pageIndex = QByteArray(argv[i] + 7).toInt();
        else if (std::strncmp(argv[i], "--lang=", 7) == 0)
            languageOverride = QString::fromLocal8Bit(argv[i] + 7);
        else if (std::strncmp(argv[i], "--screenshot=", 13) == 0)
            screenshotPath = QString::fromLocal8Bit(argv[i] + 13);
    }

    // The Wayland QPA plugin is linked statically into the executable. Prefer
    // native Wayland on a Wayland session (checked via WAYLAND_DISPLAY, so an
    // explicit QT_QPA_PLATFORM always wins) and keep xcb as the fallback.
#ifdef AUROKU_HAVE_WAYLAND_PLUGIN
    if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")
        && !qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY"))
        qputenv("QT_QPA_PLATFORM", QByteArrayLiteral("wayland;xcb"));
#endif

    installGuiLog();

    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("auRokku"));
    QApplication::setApplicationDisplayName(QStringLiteral("auRokku"));
    QApplication::setOrganizationName(QStringLiteral("auroku"));
    QApplication::setApplicationVersion(QStringLiteral(AUROKU_APP_VERSION));
    if (!QStandardPaths::locate(QStandardPaths::ApplicationsLocation,
                                 QStringLiteral("auroku.desktop")).isEmpty()) {
        QApplication::setDesktopFileName(QStringLiteral("auroku"));
    }

    // Fusion behaves predictably with style sheets on every platform.
    if (QStyle *style = QStyleFactory::create(QStringLiteral("Fusion")))
        QApplication::setStyle(style);

    Fonts::apply();

    I18n::Language language = I18n::detectLanguage();
    if (!languageOverride.isEmpty())
        language = languageOverride.startsWith(QLatin1String("zh"), Qt::CaseInsensitive)
            ? I18n::Language::ZhCn
            : I18n::Language::En;
    if (language == I18n::Language::ZhCn && !Fonts::hasCjk())
        language = I18n::Language::En;
    I18n::instance().setLanguage(language);

    Theme::instance().setDark(darkTheme);
    Theme::instance().apply();

    if (!qEnvironmentVariableIsEmpty("AUROKU_DUMP_QSS")) {
        QFile file(qEnvironmentVariable("AUROKU_DUMP_QSS"));
        if (file.open(QIODevice::WriteOnly | QIODevice::Truncate))
            file.write(Theme::instance().styleSheet().toUtf8());
    }

    qInfo().noquote() << QStringLiteral("auRokku %1 starting (Qt %2, %3)")
                             .arg(QStringLiteral(AUROKU_APP_VERSION), QString::fromLatin1(qVersion()),
                                  QSysInfo::productType());

    MainWindow window;
    window.show();

    if (pageIndex >= 0 && pageIndex <= static_cast<int>(PageId::Console))
        QTimer::singleShot(400, &window, [&window, pageIndex] {
            window.goToPage(static_cast<PageId>(pageIndex));
        });

    if (!screenshotPath.isEmpty())
        QTimer::singleShot(1400, &window, [&window, &app, screenshotPath] {
            window.grab().save(screenshotPath);
            app.quit();
        });

    if (smokeTest)
        QTimer::singleShot(1500, &app, &QCoreApplication::quit);

    return app.exec();
}
