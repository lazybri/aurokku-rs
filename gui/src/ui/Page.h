#pragma once

#include <QJsonObject>
#include <QWidget>

#include "core/PageHost.h"
#include "ui/Widgets.h"

class QPushButton;
class QVBoxLayout;

/// Base class for the seven content pages.
class Page : public QWidget {
    Q_OBJECT
public:
    explicit Page(PageHost *host, QWidget *parent = nullptr);

    /// Rebuild every dynamic (result) area from the current `AppState`.
    virtual void refresh() {}

    /// Re-apply translations and theme-dependent icons.
    void retranslate();

    /// Enable/disable operation buttons while a USB call or scan is running.
    void setBusy(bool busy);

protected:
    PageHost *host() const { return m_host; }
    AppState &state() const { return m_host->state(); }

    QPushButton *track(QPushButton *button);
    void addPageHeader(const QString &titleKey, const QString &subtitleKey);
    void addLastResultCard();
    void updateLastResult();

    QVBoxLayout *m_content = nullptr;

private:
    QWidget *buildStatusSummary(const QJsonObject &result);
    QWidget *buildGenericSummary(const QJsonObject &result);

    PageHost *m_host = nullptr;
    QVector<QPushButton *> m_actions;
    QVBoxLayout *m_lastResultBody = nullptr;
};
