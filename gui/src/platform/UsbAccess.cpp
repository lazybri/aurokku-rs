#include "platform/UsbAccess.h"

#include <QProcess>
#include <QStandardPaths>

#include <cstdio>

#if defined(Q_OS_LINUX) || defined(__linux__)
#include <sys/stat.h>
#include <unistd.h>

namespace {

struct AclEntries {
    bool hasUser = false;
    QString userPermissions;
    bool hasMask = false;
    QString maskPermissions;
};

bool run(const QString &program, const QStringList &arguments, QByteArray *output,
         QString *error)
{
    QProcess process;
    process.start(program, arguments);
    if (!process.waitForStarted(3000)) {
        *error = process.errorString();
        return false;
    }
    if (!process.waitForFinished(3000)) {
        process.kill();
        process.waitForFinished();
        *error = QStringLiteral("command timed out: %1").arg(program);
        return false;
    }
    if (output != nullptr)
        *output = process.readAllStandardOutput();
    if (process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0) {
        const QString details = QString::fromLocal8Bit(process.readAllStandardError()).trimmed();
        *error = details.isEmpty() ? QStringLiteral("command failed: %1").arg(program) : details;
        return false;
    }
    return true;
}

bool readAcl(const QString &getfacl, const QString &path, quint64 uid, AclEntries *entries,
             QString *error)
{
    QByteArray output;
    if (!run(getfacl, {QStringLiteral("-cpn"), QStringLiteral("--"), path}, &output, error))
        return false;

    const QByteArray userPrefix = "user:" + QByteArray::number(uid) + ':';
    const QList<QByteArray> lines = output.split('\n');
    for (const QByteArray &line : lines) {
        if (line.startsWith(userPrefix)) {
            entries->hasUser = true;
            entries->userPermissions = QString::fromLatin1(line.mid(userPrefix.size()));
        } else if (line.startsWith("mask::")) {
            entries->hasMask = true;
            entries->maskPermissions = QString::fromLatin1(line.mid(6));
        }
    }
    return true;
}

bool sameNode(const QString &path, quint64 deviceId, quint64 inode)
{
    struct stat status {};
    return ::stat(path.toLocal8Bit().constData(), &status) == 0
        && S_ISCHR(status.st_mode)
        && static_cast<quint64>(status.st_dev) == deviceId
        && static_cast<quint64>(status.st_ino) == inode;
}

QString systemExecutable(const QString &name)
{
    return QStandardPaths::findExecutable(
        name, {QStringLiteral("/usr/bin"), QStringLiteral("/bin"), QStringLiteral("/usr/sbin"),
               QStringLiteral("/sbin"), QStringLiteral("/run/current-system/sw/bin")});
}

bool changeAcl(const QString &pkexec, const QString &setfacl, const QStringList &arguments,
               QString *error)
{
    if (pkexec.isEmpty()) {
        *error = QStringLiteral("pkexec is required to change device ACLs");
        return false;
    }
    if (setfacl.isEmpty()) {
        *error = QStringLiteral("setfacl is missing; install the 'acl' package");
        return false;
    }
    QStringList elevatedArguments{setfacl};
    elevatedArguments.append(arguments);
    return run(pkexec, elevatedArguments, nullptr, error);
}

} // namespace
#endif

UsbAccess::~UsbAccess()
{
    QString error;
    if (!revoke(&error) && !error.isEmpty())
        std::fprintf(stderr, "[auroku-gui] failed to restore USB ACL: %s\n",
                     error.toLocal8Bit().constData());
}

void UsbAccess::clearState()
{
    m_path.clear();
    m_previousUserPermissions.clear();
    m_previousMask.clear();
    m_grantedMask.clear();
    m_deviceId = 0;
    m_inode = 0;
    m_uid = 0;
    m_bus = 0;
    m_address = 0;
    m_hadUserEntry = false;
    m_hadMask = false;
    m_restoreMaskUnconditionally = false;
    m_active = false;
}

bool UsbAccess::grant(int bus, int address, QString *error)
{
#if defined(Q_OS_LINUX) || defined(__linux__)
    if (bus < 1 || bus > 255 || address < 1 || address > 255) {
        if (error != nullptr)
            *error = QStringLiteral("invalid USB bus/address");
        return false;
    }

    const QString path = QStringLiteral("/dev/bus/usb/%1/%2")
                             .arg(bus, 3, 10, QLatin1Char('0'))
                             .arg(address, 3, 10, QLatin1Char('0'));
    struct stat status {};
    if (::stat(path.toLocal8Bit().constData(), &status) != 0 || !S_ISCHR(status.st_mode)) {
        if (error != nullptr)
            *error = QStringLiteral("USB device node is unavailable: %1").arg(path);
        return false;
    }
    if (::geteuid() == 0)
        return true;

    const quint64 deviceId = static_cast<quint64>(status.st_dev);
    const quint64 inode = static_cast<quint64>(status.st_ino);
    if (isGrantedFor(bus, address) && m_deviceId == deviceId && m_inode == inode)
        return true;

    if (!revoke(error))
        return false;

    const QString getfacl = systemExecutable(QStringLiteral("getfacl"));
    const QString setfacl = systemExecutable(QStringLiteral("setfacl"));
    const QString pkexec = systemExecutable(QStringLiteral("pkexec"));
    if (getfacl.isEmpty() || setfacl.isEmpty() || pkexec.isEmpty()) {
        if (error != nullptr)
            *error = QStringLiteral("pkexec and the 'acl' package (getfacl/setfacl) are required");
        return false;
    }

    const quint64 uid = static_cast<quint64>(::geteuid());
    AclEntries original;
    QString commandError;
    if (!readAcl(getfacl, path, uid, &original, &commandError)) {
        if (error != nullptr)
            *error = commandError;
        return false;
    }

    const QString rule = QStringLiteral("u:%1:rw-").arg(uid);
    if (!changeAcl(pkexec, setfacl,
                   {QStringLiteral("-n"), QStringLiteral("-m"), rule,
                    QStringLiteral("--"), path},
                   &commandError)) {
        if (error != nullptr)
            *error = commandError;
        return false;
    }

    auto rememberGrant = [&](const QString &grantedMask, bool restoreMaskUnconditionally) {
        m_path = path;
        m_previousUserPermissions = original.userPermissions;
        m_previousMask = original.maskPermissions;
        m_grantedMask = grantedMask;
        m_deviceId = deviceId;
        m_inode = inode;
        m_uid = uid;
        m_bus = bus;
        m_address = address;
        m_hadUserEntry = original.hasUser;
        m_hadMask = original.hasMask;
        m_restoreMaskUnconditionally = restoreMaskUnconditionally;
        m_active = true;
    };

    AclEntries granted;
    const bool aclRead = readAcl(getfacl, path, uid, &granted, &commandError);
    const bool maskAllowsReadWrite =
        granted.hasMask && granted.maskPermissions.contains(QLatin1Char('r'))
        && granted.maskPermissions.contains(QLatin1Char('w'));
    if (!aclRead || !maskAllowsReadWrite) {
        QStringList rollback;
        if (original.hasUser) {
            rollback << QStringLiteral("-m")
                     << QStringLiteral("u:%1:%2").arg(uid).arg(original.userPermissions);
        } else {
            rollback << QStringLiteral("-x") << QStringLiteral("u:%1").arg(uid);
        }
        if (original.hasMask) {
            rollback << QStringLiteral("-m")
                     << QStringLiteral("m::%1").arg(original.maskPermissions);
        } else if (granted.hasMask) {
            rollback << QStringLiteral("-x") << QStringLiteral("m::");
        }
        rollback << QStringLiteral("--") << path;
        QString rollbackError;
        const bool rolledBack = changeAcl(pkexec, setfacl, rollback, &rollbackError);
        if (!rolledBack) {
            rememberGrant(granted.maskPermissions, !aclRead);
            if (error != nullptr)
                *error = QStringLiteral("%1; ACL rollback failed: %2")
                             .arg(aclRead
                                      ? QStringLiteral("the existing ACL mask prevents read/write access")
                                      : commandError,
                                  rollbackError);
            return false;
        }
        if (error != nullptr)
            *error = aclRead
                ? QStringLiteral("the existing ACL mask prevents read/write access; no broader ACL was applied")
                : commandError;
        return false;
    }

    rememberGrant(granted.maskPermissions, false);
#else
    Q_UNUSED(bus);
    Q_UNUSED(address);
    Q_UNUSED(error);
#endif
    return true;
}

bool UsbAccess::revoke(QString *error)
{
#if defined(Q_OS_LINUX) || defined(__linux__)
    if (!m_active)
        return true;
    if (!sameNode(m_path, m_deviceId, m_inode)) {
        m_active = false;
        return true;
    }

    const QString getfacl = systemExecutable(QStringLiteral("getfacl"));
    const QString setfacl = systemExecutable(QStringLiteral("setfacl"));
    const QString pkexec = systemExecutable(QStringLiteral("pkexec"));
    if (getfacl.isEmpty() || setfacl.isEmpty() || pkexec.isEmpty()) {
        if (error != nullptr)
            *error = QStringLiteral("pkexec and the 'acl' package are required to restore USB permissions");
        return false;
    }

    AclEntries current;
    QString commandError;
    if (!readAcl(getfacl, m_path, m_uid, &current, &commandError)) {
        if (error != nullptr)
            *error = commandError;
        return false;
    }

    QStringList arguments;
    if (m_hadUserEntry) {
        arguments << QStringLiteral("-m")
                  << QStringLiteral("u:%1:%2").arg(m_uid).arg(m_previousUserPermissions);
    } else {
        arguments << QStringLiteral("-x") << QStringLiteral("u:%1").arg(m_uid);
    }

    if (m_restoreMaskUnconditionally
        || (current.hasMask && current.maskPermissions == m_grantedMask)) {
        if (m_hadMask) {
            arguments << QStringLiteral("-m")
                      << QStringLiteral("m::%1").arg(m_previousMask);
        } else if (current.hasMask) {
            arguments << QStringLiteral("-x") << QStringLiteral("m::");
        }
    }

    arguments << QStringLiteral("--") << m_path;
    if (!changeAcl(pkexec, setfacl, arguments, &commandError)) {
        if (error != nullptr)
            *error = commandError;
        return false;
    }
    clearState();
#else
    Q_UNUSED(error);
#endif
    return true;
}

bool UsbAccess::isGrantedFor(int bus, int address) const
{
#if defined(Q_OS_LINUX) || defined(__linux__)
    if (::geteuid() == 0)
        return true;
    return m_active && bus == m_bus && address == m_address
        && sameNode(m_path, m_deviceId, m_inode);
#else
    Q_UNUSED(bus);
    Q_UNUSED(address);
    return true;
#endif
}
