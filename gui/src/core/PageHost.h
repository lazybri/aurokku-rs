#pragma once

#include <functional>

#include "AppState.h"

class RustBridge;

/// Navigation destinations for the seven content pages.
enum class PageId { Overview, Uim, Pin, Probe, Batch, Session, Console };

/// Interface through which pages talk to the main window.
class PageHost {
public:
    virtual ~PageHost() = default;

    virtual AppState &state() = 0;
    virtual RustBridge &bridge() = 0;

    /// Validate the device form and queue a protocol operation.
    virtual void startTask(const QJsonObject &task, const QString &labelKey) = 0;

    /// Enumerate the USB bus; `filtered` applies the VID/PID form fields.
    virtual void scanDevices(bool filtered) = 0;

    /// Copy a scanned device into the device form.
    virtual void selectDevice(int index, bool announce) = 0;

    /// Show a transient toast in the bottom-right corner.
    virtual void showToast(const QString &text, Tone tone) = 0;

    /// Ask for confirmation before running `action`.
    virtual void confirmAction(const QString &titleKey, const QString &body, const QString &actionKey,
                               const std::function<void()> &action) = 0;

    virtual void goToPage(PageId page) = 0;

    /// True on Linux when running as root (or on platforms without elevation).
    virtual bool elevated() const = 0;
};
