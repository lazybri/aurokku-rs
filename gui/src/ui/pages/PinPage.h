#pragma once

#include "ui/Page.h"

class QVBoxLayout;
class QWidget;

/// Phone lock code (NV 82) and ICCID record 0x59.
class PinPage : public Page {
    Q_OBJECT
public:
    explicit PinPage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

private:
    void buildReadCard();
    void buildResetCard();
    void buildIccidCard();
    void updateCards();

    QVBoxLayout *m_readLayout = nullptr;
    QWidget *m_readHost = nullptr;
    QVBoxLayout *m_resetLayout = nullptr;
    QWidget *m_resetHost = nullptr;
    QVBoxLayout *m_iccidLayout = nullptr;
    QWidget *m_iccidHost = nullptr;
};
