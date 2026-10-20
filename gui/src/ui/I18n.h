#pragma once

#include <QObject>
#include <QString>
#include <QVariantMap>

/// Runtime localisation (Simplified Chinese / English).
///
/// The UI is written with Chinese source strings. Labels authored through the
/// `Ui` helpers remember their source string in an `i18nKey` property, so
/// `Ui::retranslateTree()` can refresh a whole page when the language changes.
class I18n : public QObject {
    Q_OBJECT
public:
    enum class Language { ZhCn, En };

    static I18n &instance();

    Language language() const;
    void setLanguage(Language language);

    /// Translate a Chinese source string; unknown strings pass through.
    static QString t(const QString &source);
    /// Translate a template and substitute `{key}` placeholders.
    static QString tf(const QString &source, const QVariantMap &arguments);

    /// `zh_CN` when the system locale is Chinese, `en` otherwise.
    static Language detectLanguage();

    QString nativeName() const;

signals:
    void languageChanged();

private:
    explicit I18n(QObject *parent = nullptr);
};
