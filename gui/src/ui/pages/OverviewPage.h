#pragma once

#include "ui/Page.h"

class QComboBox;
class QLabel;
class QLineEdit;
class QPushButton;
class QVBoxLayout;
class ToggleSwitch;

/// Device selection, transport options and quick actions.
class OverviewPage : public Page {
    Q_OBJECT
public:
    explicit OverviewPage(PageHost *host, QWidget *parent = nullptr);

    void refresh() override;

    /// Copy `AppState` device fields into the form (after a device is picked).
    void syncFormFromState();

private:
    void buildConnectionCard();
    void buildQuickActionsCard();
    void persistForm();
    void rebuildDeviceCombo();
    void updateScanError();

    QComboBox *m_deviceCombo = nullptr;
    QLineEdit *m_vid = nullptr;
    QLineEdit *m_pid = nullptr;
    QLineEdit *m_sn = nullptr;
    QLineEdit *m_interface = nullptr;
    QLineEdit *m_epOut = nullptr;
    QLineEdit *m_epIn = nullptr;
    ToggleSwitch *m_type2 = nullptr;
    ToggleSwitch *m_verbose = nullptr;
    QPushButton *m_scanAll = nullptr;
    QPushButton *m_scanFiltered = nullptr;
    QLabel *m_busyLabel = nullptr;
    QWidget *m_scanErrorHost = nullptr;
    QVBoxLayout *m_scanErrorLayout = nullptr;
    QString m_deviceSignature;
    bool m_comboPlaceholder = false;
};
