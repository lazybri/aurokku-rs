//! auRokku Qt front-end entry point.

#include <QApplication>
#include <QByteArray>
#include <QCoreApplication>
#include <QFile>
#include <QStyleFactory>
#include <QTimer>
#include <cstring>

#include "platform/Elevate.h"
#include "ui/Fonts.h"
#include "ui/I18n.h"
#include "ui/MainWindow.h"
#include "ui/Theme.h"

#ifndef AUROKU_APP_VERSION
#define AUROKU_APP_VERSION "0.1.0"
#endif

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

    // Elevate before QApplication so the elevated child owns the display
    // connection from the start (Linux only; no-op elsewhere).
    Elevate::maybeElevate(argc, argv);

    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("auRokku"));
    QApplication::setApplicationDisplayName(QStringLiteral("auRokku"));
    QApplication::setOrganizationName(QStringLiteral("auroku"));
    QApplication::setApplicationVersion(QStringLiteral(AUROKU_APP_VERSION));
    QApplication::setDesktopFileName(QStringLiteral("auroku"));

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
