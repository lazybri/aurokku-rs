#pragma once

#include "ui/Page.h"

class QLabel;
class QLineEdit;
class QVBoxLayout;
class ToggleSwitch;

/// UIM lock read / write / clear and the NV 10481 maintenance bit.
class UimPage : public Page {
    Q_OBJECT
public:
    explicit UimPage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

private:
    void buildReadCard();
    void buildWriteCard();
    void buildClearCard();
    void buildMaintenanceCard();
    void updateReadResult();

    QVBoxLayout *m_readResultLayout = nullptr;
    QWidget *m_readResultHost = nullptr;
    QLineEdit *m_writeValue = nullptr;
    ToggleSwitch *m_writeControl = nullptr;
    QLabel *m_writeControlValue = nullptr;
    ToggleSwitch *m_maintControl = nullptr;
    QLabel *m_maintControlValue = nullptr;
};
