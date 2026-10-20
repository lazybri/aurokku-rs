#pragma once

#include "ui/Page.h"

class QPushButton;
class QVBoxLayout;
class QWidget;
class SegmentedControl;

/// Batch queries inside a single session.
class BatchPage : public Page {
    Q_OBJECT
public:
    explicit BatchPage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

private:
    void renderResult();

    SegmentedControl *m_profile = nullptr;
    QPushButton *m_run = nullptr;
    QWidget *m_resultHost = nullptr;
    QVBoxLayout *m_resultLayout = nullptr;
};
