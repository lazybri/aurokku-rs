#include "platform/Elevate.h"

#include <cstdio>

#if defined(Q_OS_LINUX) || defined(__linux__)

#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "ui/I18n.h"

namespace {

const char *const kForwardedEnv[] = {
    "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_DESKTOP", "DISPLAY", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS",
    "QT_QPA_PLATFORM", "QT_PLUGIN_PATH", "LANG", "LC_ALL", "LC_CTYPE", "LC_MESSAGES",
    "AUROKU_GUI_FONT", "RUST_LOG", "RUST_BACKTRACE",
};

const char *const kPkexecCandidates[] = {"/usr/bin/pkexec", "/bin/pkexec", "/usr/sbin/pkexec",
                                         "/run/current-system/sw/bin/pkexec"};
const char *const kEnvCandidates[] = {"/usr/bin/env", "/bin/env"};

bool optedOut(int argc, char **argv)
{
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--no-polkit") == 0)
            return true;
    }
    const char *value = std::getenv("AUROKU_GUI_NO_POLKIT");
    if (value == nullptr || *value == '\0')
        return false;
    const std::string text(value);
    return !(text == "0" || text == "false" || text == "FALSE");
}

bool alreadyElevated()
{
    return std::getenv("AUROKU_GUI_ELEVATED") != nullptr || std::getenv("PKEXEC_UID") != nullptr;
}

bool worldWritable(const std::string &path)
{
    struct stat info {};
    if (::stat(path.c_str(), &info) != 0)
        return false;
    return S_ISREG(info.st_mode) && (info.st_mode & S_IWOTH) != 0;
}

std::string findProgram(const char *const *candidates, std::size_t count)
{
    for (std::size_t i = 0; i < count; ++i) {
        if (::access(candidates[i], X_OK) == 0)
            return candidates[i];
    }
    return {};
}

bool forwardable(const std::string &name)
{
    for (const char *allowed : kForwardedEnv) {
        if (name == allowed)
            return true;
    }
    return name.rfind("LC_", 0) == 0;
}

std::vector<std::string> forwardedEnv()
{
    std::vector<std::string> out;
    for (char **entry = environ; *entry != nullptr; ++entry) {
        const std::string pair(*entry);
        const auto equals = pair.find('=');
        if (equals == std::string::npos)
            continue;
        if (forwardable(pair.substr(0, equals)))
            out.push_back(pair);
    }

    if (std::getenv("XAUTHORITY") == nullptr) {
        if (const char *home = std::getenv("HOME")) {
            const std::string candidate = std::string(home) + "/.Xauthority";
            struct stat info {};
            if (::stat(candidate.c_str(), &info) == 0 && S_ISREG(info.st_mode))
                out.push_back("XAUTHORITY=" + candidate);
        }
    }
    return out;
}

} // namespace

bool Elevate::isEffectiveRoot()
{
    static const bool root = ::geteuid() == 0;
    return root;
}

void Elevate::maybeElevate(int argc, char **argv)
{
    if (isEffectiveRoot() || optedOut(argc, argv) || alreadyElevated())
        return;

    // A root client usually cannot connect to the user's Wayland socket.
    // Skip automatic elevation on pure Wayland sessions and let the app run
    // unprivileged (udev rules) or with QT_QPA_PLATFORM=xcb.
    if (std::getenv("WAYLAND_DISPLAY") != nullptr && std::getenv("DISPLAY") == nullptr) {
        std::fprintf(stderr, "[auroku-gui] %s\n",
                     qPrintable(I18n::t(QStringLiteral(
                         "检测到纯 Wayland 会话，已跳过 polkit 提权（root 客户端通常无法连接 Wayland）。可设置 QT_QPA_PLATFORM=xcb 或配置 udev 规则。"))));
        return;
    }

    char executable[4096];
    const ssize_t length = ::readlink("/proc/self/exe", executable, sizeof(executable) - 1);
    if (length <= 0) {
        std::fprintf(stderr, "[auroku-gui] %s\n",
                     qPrintable(I18n::t(QStringLiteral("可执行文件路径无效；将以普通用户运行"))));
        return;
    }
    executable[length] = '\0';
    const std::string exePath(executable);

    if (worldWritable(exePath)) {
        std::fprintf(stderr, "[auroku-gui] %s\n",
                     qPrintable(I18n::t(QStringLiteral(
                         "可执行文件对任意用户可写，拒绝提权；请修复文件权限后重试"))));
        return;
    }

    const std::string pkexec =
        findProgram(kPkexecCandidates, sizeof(kPkexecCandidates) / sizeof(kPkexecCandidates[0]));
    if (pkexec.empty()) {
        std::fprintf(stderr, "[auroku-gui] %s\n",
                     qPrintable(I18n::t(
                         QStringLiteral("未找到 pkexec（polkit），将以普通用户运行"))));
        return;
    }

    std::vector<std::string> arguments;
    arguments.push_back(pkexec);

    const std::string envProgram =
        findProgram(kEnvCandidates, sizeof(kEnvCandidates) / sizeof(kEnvCandidates[0]));
    if (!envProgram.empty()) {
        arguments.push_back(envProgram);
        for (const std::string &pair : forwardedEnv())
            arguments.push_back(pair);
        arguments.push_back("AUROKU_GUI_ELEVATED=1");
    }

    arguments.push_back(exePath);
    for (int i = 1; i < argc; ++i) {
        if (std::strcmp(argv[i], "--no-polkit") != 0)
            arguments.push_back(argv[i]);
    }

    std::fprintf(stderr, "[auroku-gui] %s\n",
                 qPrintable(I18n::t(QStringLiteral("正在通过 polkit 请求 root 权限…"))));
    std::fflush(stderr);

    std::vector<char *> raw;
    raw.reserve(arguments.size() + 1);
    for (std::string &argument : arguments)
        raw.push_back(argument.data());
    raw.push_back(nullptr);

    const pid_t child = ::fork();
    if (child == 0) {
        ::execv(pkexec.c_str(), raw.data());
        std::_Exit(127);
    }
    if (child < 0) {
        std::fprintf(stderr, "[auroku-gui] %s\n",
                     qPrintable(I18n::tf(QStringLiteral("无法启动 pkexec：{error}；将以普通用户运行"),
                                         {{QStringLiteral("error"), QString::fromLocal8Bit(std::strerror(errno))}})));
        return;
    }

    int status = 0;
    if (::waitpid(child, &status, 0) < 0)
        return;

    if (WIFEXITED(status)) {
        const int code = WEXITSTATUS(status);
        if (code >= 125 && code <= 127) {
            std::fprintf(stderr, "[auroku-gui] %s\n",
                         qPrintable(I18n::t(QStringLiteral(
                             "polkit 认证未完成，将以普通用户继续运行（USB 访问可能受限）"))));
            return;
        }
        std::_Exit(code);
    }
    std::_Exit(1);
}

#else // !Linux

bool Elevate::isEffectiveRoot()
{
    return true;
}

void Elevate::maybeElevate(int, char **)
{
}

#endif
