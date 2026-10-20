#pragma once

#include "ui/Page.h"

class QVBoxLayout;
class QWidget;

/// Service exit commands and the resulting session record.
class SessionPage : public Page {
    Q_OBJECT
public:
    explicit SessionPage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

private:
    void renderDetails();

    QWidget *m_detailsHost = nullptr;
    QVBoxLayout *m_detailsLayout = nullptr;
};
