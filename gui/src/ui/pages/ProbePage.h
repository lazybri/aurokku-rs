#pragma once

#include "ui/Page.h"

class QLineEdit;
class QPushButton;
class QVBoxLayout;
class QWidget;
class SegmentedControl;

/// USB layout / SERI echo / SCDP probing.
class ProbePage : public Page {
    Q_OBJECT
public:
    explicit ProbePage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

private:
    void startProbe();
    void renderResult();
    void renderProbe(const QJsonObject &result);

    SegmentedControl *m_mode = nullptr;
    QLineEdit *m_timeout = nullptr;
    QPushButton *m_run = nullptr;
    QWidget *m_resultHost = nullptr;
    QVBoxLayout *m_resultLayout = nullptr;
};
