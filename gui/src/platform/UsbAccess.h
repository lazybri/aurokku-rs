#pragma once

#include <QString>

class UsbAccess {
public:
    UsbAccess() = default;
    ~UsbAccess();

    UsbAccess(const UsbAccess &) = delete;
    UsbAccess &operator=(const UsbAccess &) = delete;

    bool grant(int bus, int address, QString *error);
    bool revoke(QString *error = nullptr);
    bool isGrantedFor(int bus, int address) const;

private:
    void clearState();

    QString m_path;
    QString m_previousUserPermissions;
    QString m_previousMask;
    QString m_grantedMask;
    quint64 m_deviceId = 0;
    quint64 m_inode = 0;
    quint64 m_uid = 0;
    int m_bus = 0;
    int m_address = 0;
    bool m_hadUserEntry = false;
    bool m_hadMask = false;
    bool m_restoreMaskUnconditionally = false;
    bool m_active = false;
};
