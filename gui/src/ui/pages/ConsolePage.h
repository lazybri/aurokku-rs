#pragma once

#include "ui/Page.h"

class QLabel;
class QPlainTextEdit;
class QPushButton;
class QStackedWidget;
class QTextEdit;
class ToggleSwitch;

/// USB traffic log and the full JSON of the latest operation.
class ConsolePage : public Page {
    Q_OBJECT
public:
    explicit ConsolePage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

    /// Append one line as it arrives (keeps the view cheap during verbose runs).
    void appendLine(const LogLine &line);

private:
    void rebuildLog();
    void rebuildJson();
    void updateCounter();
    QString lineHtml(const LogLine &line) const;

    ToggleSwitch *m_verbose = nullptr;
    ToggleSwitch *m_autoScroll = nullptr;
    QLabel *m_count = nullptr;
    QTextEdit *m_logView = nullptr;
    QStackedWidget *m_jsonStack = nullptr;
    QPlainTextEdit *m_jsonView = nullptr;
};
