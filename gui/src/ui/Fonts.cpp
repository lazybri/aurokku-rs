#include "Fonts.h"

#include <QApplication>
#include <QFileInfo>
#include <QFont>
#include <QFontDatabase>
#include <QStringList>

namespace {

QString pickFamily(const QStringList &candidates)
{
    const QStringList installed = QFontDatabase::families();
    for (const QString &candidate : candidates) {
        for (const QString &family : installed) {
            if (family.compare(candidate, Qt::CaseInsensitive) == 0)
                return family;
        }
    }
    return {};
}

QStringList cjkFamilies()
{
    return {QStringLiteral("Microsoft YaHei UI"), QStringLiteral("Microsoft YaHei"),
            QStringLiteral("Noto Sans CJK SC"), QStringLiteral("Noto Sans SC"),
            QStringLiteral("Source Han Sans SC"), QStringLiteral("PingFang SC"),
            QStringLiteral("Hiragino Sans GB"), QStringLiteral("WenQuanYi Micro Hei"),
            QStringLiteral("Droid Sans Fallback")};
}

} // namespace

QString Fonts::uiFamily()
{
    // An explicit override wins: either a family name or a font file.
    const QString override = qEnvironmentVariable("AUROKU_GUI_FONT");
    if (!override.isEmpty()) {
        if (QFileInfo::exists(override)) {
            const int id = QFontDatabase::addApplicationFont(override);
            const QStringList families = QFontDatabase::applicationFontFamilies(id);
            if (!families.isEmpty())
                return families.first();
        } else {
            return override;
        }
    }

    if (const QString family = pickFamily(cjkFamilies()); !family.isEmpty())
        return family;

    const QString fallback =
        pickFamily({QStringLiteral("Segoe UI"), QStringLiteral("Noto Sans"),
                    QStringLiteral("DejaVu Sans"), QStringLiteral("Helvetica Neue")});
    return fallback;
}

QString Fonts::monospaceFamily()
{
    const QString family =
        pickFamily({QStringLiteral("Cascadia Mono"), QStringLiteral("Cascadia Code"),
                    QStringLiteral("JetBrains Mono"), QStringLiteral("Consolas"),
                    QStringLiteral("Noto Sans Mono CJK SC"), QStringLiteral("Source Han Mono SC"),
                    QStringLiteral("DejaVu Sans Mono"), QStringLiteral("Liberation Mono")});
    if (!family.isEmpty())
        return family;
    return QFontDatabase::systemFont(QFontDatabase::FixedFont).family();
}

bool Fonts::hasCjk()
{
    const QStringList installed = QFontDatabase::families();
    for (const QString &known : cjkFamilies()) {
        for (const QString &family : installed) {
            if (family.compare(known, Qt::CaseInsensitive) == 0)
                return true;
        }
    }
    return false;
}

void Fonts::apply()
{
    QFont font(uiFamily());
    font.setPixelSize(13);
    font.setStyleStrategy(QFont::PreferAntialias);
    QApplication::setFont(font);
}
