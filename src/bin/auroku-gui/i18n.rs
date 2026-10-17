//! Minimal runtime localisation (Simplified Chinese / English).
//!
//! The UI is built from Chinese source strings. Components pass their labels
//! through [`t`] / [`tf`] so switching the language only changes one global
//! setting; no page needs to be duplicated.

use std::sync::atomic::{AtomicU8, Ordering};

/// Supported interface languages.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Language {
    ZhCn = 0,
    En = 1,
}

impl Language {
    /// Name shown in the language selector.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::ZhCn => "中文",
            Language::En => "English",
        }
    }

    /// Pick a default from `LC_ALL` / `LC_MESSAGES` / `LANG`.
    ///
    /// An unset locale keeps the historical Chinese default; an explicit
    /// non-Chinese locale starts in English.
    pub fn from_env() -> Self {
        let locale = std::env::var("LC_ALL")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var("LC_MESSAGES")
                    .ok()
                    .filter(|value| !value.is_empty())
            })
            .or_else(|| std::env::var("LANG").ok().filter(|value| !value.is_empty()));

        match locale {
            Some(locale) if locale.to_ascii_lowercase().starts_with("zh") => Language::ZhCn,
            Some(_) => Language::En,
            None => Language::ZhCn,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(Language::ZhCn as u8);

/// Set the process-wide interface language.
pub fn set(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

/// Current interface language.
pub fn current() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::En,
        _ => Language::ZhCn,
    }
}

/// Translate a static UI string. Unknown strings pass through unchanged.
pub fn t(text: &str) -> &str {
    if current() == Language::ZhCn {
        return text;
    }
    match text {
        // ------------------------------------------------------------------
        // Navigation / app chrome
        // ------------------------------------------------------------------
        "概览" => "Overview",
        "UIM 锁" => "UIM Lock",
        "探测" => "Probe",
        "批量" => "Batch",
        "会话" => "Session",
        "控制台" => "Console",
        "关于" => "About",
        "切换浅色 / 深色主题" => "Toggle light / dark theme",
        "SERI / Type2Auth 设备工具" => "SERI / Type2Auth device tool",
        "执行中" => "Running",
        "错误" => "Error",
        "就绪" => "Ready",
        "失败" => "Failed",
        "待命" => "Idle",
        "权限：root" => "Privilege: root",
        "权限：普通用户 · USB 访问可能受限" => {
            "Privilege: user · USB access may be limited"
        }
        "USB 访问可能需要管理员权限" => "USB access may require administrator privileges",
        "auRokku v{version} · 基于Yuu所提供的代码二次开发 ©lazybri" => {
            "auRokku v{version} · Based on code by Yuu, further developed ©lazybri"
        }
        "界面语言已切换为 {lang}" => "Interface language: {lang}",
        "当前操作不可用" => "This action is currently unavailable",
        "未找到 CJK 字体，无法显示中文界面" => {
            "No CJK font found; the Chinese UI is unavailable"
        }
        "取消" => "Cancel",
        "关闭" => "Close",

        // ------------------------------------------------------------------
        // Overview
        // ------------------------------------------------------------------
        "连接 au / SERI 设备，读取锁状态、PIN 与 SIM 信息" => {
            "Connect to an au / SERI device and read lock state, PIN and SIM information"
        }
        "当前以普通用户运行，USB 访问可能失败。可使用 pkexec 重新启动并完成 polkit 认证，或配置 udev 规则。" => {
            "Running as a normal user; USB access may fail. Restart via pkexec and complete polkit authentication, or configure a udev rule."
        }
        "选择已扫描设备…" => "Select a scanned device…",
        "设备连接" => "Device connection",
        "扫描 USB 总线并选择目标设备" => "Scan the USB bus and select a target device",
        "扫描中…" => "Scanning…",
        "扫描全部" => "Scan all",
        "未发现设备（请检查 USB 权限后重新扫描）" => {
            "No devices found (check USB permissions and rescan)"
        }
        "SN（可选）" => "SN (optional)",
        "SKYKA077394 或 16 位 hex" => "SKYKA077394 or 16 hex digits",
        "接口号（可选）" => "Interface (optional)",
        "自动" => "Auto",
        "Type2Auth 52/53 认证" => "Type2Auth 52/53 authentication",
        "详细 USB 日志" => "Verbose USB log",
        "SN 留空时读取设备 iSerialNumber；若设备不提供序列号，请手动填写 11 位 SN 或 16 位十六进制。" => {
            "When SN is empty the device iSerialNumber is used. If the device has no serial, enter an 11-character SN or 16 hex digits manually."
        }
        "快捷操作" => "Quick actions",
        "所有 USB 操作都在后台线程执行，界面不会卡住" => {
            "All USB operations run on a background thread, so the UI stays responsive"
        }
        "读取综合状态" => "Read full status",
        "USB 布局探测" => "USB layout probe",
        "退出服务 (0xD8)" => "Exit service (0xD8)",
        "打开日志控制台" => "Open log console",
        "正在执行：{label}" => "Running: {label}",
        "最近结果" => "Latest result",
        "最近一次操作的结构化摘要" => "Structured summary of the latest operation",
        "成功" => "Success",
        "查看完整 JSON" => "View full JSON",
        "还没有结果" => "No result yet",
        "运行一次状态读取或探测后，结果会显示在这里" => {
            "Run a status read or probe and the result will appear here"
        }
        "设备 SN" => "Device SN",
        "手机 PIN" => "Phone PIN",
        "Base 版本 (raw)" => "Base version (raw)",
        "已解锁 (ffffffff)" => "Unlocked (ffffffff)",
        "已锁定 ({value})" => "Locked ({value})",

        // Generic result keys / values
        "UIM 锁值" => "UIM lock value",
        "控制位" => "Control bit",
        "解码数据" => "Decoded data",
        "原始数据" => "Raw data",
        "应答" => "Reply",
        "请求" => "Request",
        "响应" => "Response",
        "维护位" => "Maintenance bit",
        "退出方式" => "Exit method",
        "写入校验" => "Write verification",
        "通过" => "Passed",
        "未通过" => "Failed",
        "命令接受" => "Command accepted",
        "是" => "Yes",
        "否" => "No",
        "该操作没有可展开的摘要字段，请查看完整 JSON。" => {
            "This operation has no expandable summary fields; view the full JSON."
        }

        // ------------------------------------------------------------------
        // UIM lock
        // ------------------------------------------------------------------
        "读取、写入或清除 SIM 锁绑定（0x54 / 0x72 / 0x57）" => {
            "Read, write or clear the SIM lock binding (0x54 / 0x72 / 0x57)"
        }
        "写入或清除锁会修改 SIM 绑定状态。执行前请确认设备与卡片匹配，避免写入错误绑定值。" => {
            "Writing or clearing the lock modifies the SIM binding state. Verify the device and card before continuing to avoid writing an incorrect binding value."
        }
        "读取锁状态" => "Read lock state",
        "进入 UIMLK 会话并读取 control 位与绑定值" => {
            "Enter a UIMLK session and read the control bit and binding value"
        }
        "读取 UIM 锁" => "Read UIM lock",
        "写入绑定值" => "Write binding value",
        "0x72：写 4 字节锁值并校验回读" => {
            "0x72: write a 4-byte lock value and verify it by reading back"
        }
        "锁值（8 位 hex）" => "Lock value (8 hex digits)",
        "清除绑定" => "Clear binding",
        "0x57：把锁值写为 ffffffff（解除绑定）" => {
            "0x57: write ffffffff to the lock value (remove binding)"
        }
        "清除 UIM 锁绑定" => "Clear UIM lock binding",
        "写入 UIM 锁绑定" => "Write UIM lock binding",
        "设置维护模式位" => "Set maintenance mode bit",
        "命令接受后会回读锁值并校验是否为 ffffffff。" => {
            "After the command is accepted, the lock value is read back and verified as ffffffff."
        }
        "维护模式位 (NV 10481)" => "Maintenance mode bit (NV 10481)",
        "0xE1 服务 + auth 4000 认证后写入" => "Written after service 0xE1 + auth 4000",
        "应用维护位" => "Apply maintenance bit",
        "需要有效的 SN / Type2Auth 材料；认证失败时会返回 0x53 校验错误。" => {
            "A valid SN / Type2Auth material is required; authentication failure returns a 0x53 verification error."
        }
        "已解锁 / 已清除" => "Unlocked / cleared",
        "已锁定" => "Locked",
        "锁值" => "Lock value",
        "原始应答" => "Raw reply",
        "写入前" => "Before write",
        "写入后" => "After write",
        "回读校验" => "Read-back verification",

        // ------------------------------------------------------------------
        // PIN / ICCID
        // ------------------------------------------------------------------
        "读取或重置手机锁码（NV 82），读取 ICCID record 0x59" => {
            "Read or reset the phone lock code (NV 82) and read ICCID record 0x59"
        }
        "读取手机锁码" => "Read phone lock code",
        "SERI 0xDC 0x00，按 0xAA 终止符解析" => {
            "SERI 0xDC 0x00, parsed by the 0xAA terminator"
        }
        "读取 PIN" => "Read PIN",
        "手机锁码" => "Phone lock code",
        "重置手机锁码" => "Reset phone lock code",
        "SERI 0xDC 0x01，恢复默认值 1234 并回读校验" => {
            "SERI 0xDC 0x01: restore the default 1234 and verify by reading back"
        }
        "重置 PIN 为 1234" => "Reset PIN to 1234",
        "回读 PIN" => "Read-back PIN",
        "1234 校验" => "1234 verification",
        "ICCID 记录" => "ICCID record",
        "进入 UIMLK 会话读取 record 0x59" => {
            "Enter a UIMLK session and read record 0x59"
        }
        "读取 ICCID" => "Read ICCID",
        "解码 ICCID" => "Decoded ICCID",

        // ------------------------------------------------------------------
        // Probe
        // ------------------------------------------------------------------
        "枚举 USB 布局，或主动探测 SERI / SCDP 服务" => {
            "Enumerate the USB layout or actively probe the SERI / SCDP services"
        }
        "探测模式" => "Probe mode",
        "list 只枚举配置；seri / scdp 会实际收发 USB 数据" => {
            "list only enumerates configurations; seri / scdp actually send and receive USB data"
        }
        "布局 list" => "Layout list",
        "SERI 回声" => "SERI echo",
        "探测超时（秒）" => "Probe timeout (seconds)",
        "开始探测" => "Start probe",
        "SERI 探测会发送 1C 05；非 0FCE 设备在成功后自动发送 D8 清理。" => {
            "SERI probe sends 1C 05; non-0FCE devices automatically send D8 cleanup on success."
        }
        "USB 布局" => "USB layout",
        "配置 / 接口 / 端点与候选 bulk 端点对" => {
            "Configurations / interfaces / endpoints and candidate bulk endpoint pairs"
        }
        "尚无探测结果" => "No probe result yet",
        "运行布局探测后，这里会显示 USB 配置树与端点信息" => {
            "Run a layout probe to see the USB configuration tree and endpoint details here"
        }
        "SERI 回声匹配" => "SERI echo matched",
        "设备有响应" => "Device responded",
        "选中接口" => "Selected interface",
        "<超时>" => "<timeout>",
        "清理命令" => "Cleanup command",
        "设备 {vid}:{pid}" => "Device {vid}:{pid}",
        "配置 {value}" => "Config {value}",
        "活动" => "Active",
        "接口 {number} · alt {alt}" => "Interface {number} · alt {alt}",
        "未发现 bulk 端点对" => "No bulk endpoint pairs found",
        "候选 bulk 端点对" => "Candidate bulk endpoint pairs",
        "SERI 回声探测" => "SERI echo probe",
        "SCDP 探测" => "SCDP probe",

        // ------------------------------------------------------------------
        // Batch
        // ------------------------------------------------------------------
        "批量查询" => "Batch query",
        "在单个会话内连续执行多条查询，避免反复进入 / 退出服务" => {
            "Run multiple queries in one session to avoid repeated service entry/exit"
        }
        "批量配置" => "Batch profile",
        "basic：版本 + 锁码；uimlk：锁状态 + record59" => {
            "basic: version + lock code; uimlk: lock state + record59"
        }
        "基础 basic" => "Basic",
        "UIMLK uimlk" => "UIMLK",
        "执行批量查询" => "Run batch query",
        "uimlk profile 需要先完成 UIMLK 认证，SN 必须可用。" => {
            "The uimlk profile requires UIMLK authentication and a usable SN."
        }
        "查询结果" => "Query results",
        "每条查询的原始字段与解码值" => "Raw fields and decoded values for each query",
        "查询：{query}" => "Query: {query}",
        "尚无批量结果" => "No batch result yet",
        "选择 profile 并执行查询后，这里会列出每条结果" => {
            "Select a profile and run the query to list each result here"
        }

        // ------------------------------------------------------------------
        // Session
        // ------------------------------------------------------------------
        "会话与服务" => "Session & services",
        "退出维护 / UIM 服务，让设备回到正常模式" => {
            "Exit maintenance / UIM services and return the device to normal mode"
        }
        "退出基础服务" => "Exit base service",
        "发送 0xD8，恢复手机键盘 / 正常界面" => {
            "Send 0xD8 to restore the phone keypad / normal UI"
        }
        "对 0FCE 设备不会发送 0xD8（与 Python 版一致）。" => {
            "0xD8 is not sent to 0FCE devices (same as the Python version)."
        }
        "退出 UIMLK 子服务" => "Exit UIMLK sub-service",
        "重新进入 UIMLK 并发送 0x56，设备可能重启" => {
            "Re-enter UIMLK and send 0x56; the device may reboot"
        }
        "退出 UIMLK (0x56)" => "Exit UIMLK (0x56)",
        "返回结果中的 exit 字段会记录 acknowledged / reply / may_reboot。" => {
            "The exit field records acknowledged / reply / may_reboot."
        }
        "退出服务" => "Exit service",
        "退出 UIMLK" => "Exit UIMLK",
        "会话详情" => "Session details",
        "退出应答与异步事件" => "Exit reply and asynchronous events",
        "退出已确认" => "Exit acknowledged",
        "退出未确认" => "Exit not acknowledged",
        "可能重启" => "May reboot",
        "本次操作没有产生服务退出记录。" => {
            "This operation produced no service exit record."
        }
        "异步事件" => "Async events",
        "还没有会话记录" => "No session record yet",
        "执行退出服务后，应答信息会显示在这里" => {
            "Run an exit service operation and the reply will appear here"
        }

        // ------------------------------------------------------------------
        // Console
        // ------------------------------------------------------------------
        "USB 收发日志与最近一次操作的完整 JSON" => {
            "USB traffic log and the full JSON of the latest operation"
        }
        "自动滚动" => "Auto-scroll",
        "清空日志" => "Clear log",
        "{count} 条" => "{count} entries",
        "暂无日志。启用“详细 USB 日志”可看到 TX / RX 帧。" => {
            "No logs yet. Enable “Verbose USB log” to see TX / RX frames."
        }
        "日志已清空" => "Log cleared",
        "完整 JSON" => "Full JSON",
        "最近一次操作" => "Latest operation",
        "复制" => "Copy",
        "没有 JSON 结果" => "No JSON result",
        "执行一次操作后，完整协议结果会显示在这里" => {
            "Run an operation and the full protocol result will appear here"
        }
        "JSON 已复制到剪贴板" => "JSON copied to clipboard",

        // ------------------------------------------------------------------
        // Confirm dialogs
        // ------------------------------------------------------------------
        "清除 UIM 锁绑定？" => "Clear UIM lock binding?",
        "重置手机锁码？" => "Reset phone lock code?",
        "写入 UIM 锁绑定？" => "Write UIM lock binding?",
        "设置维护模式位？" => "Set maintenance mode bit?",
        "此操作会把锁值写为 ffffffff 并解除 SIM 绑定。请确认设备与卡片状态后再继续。" => {
            "This writes ffffffff to the lock value and removes the SIM binding. Verify the device and card state before continuing."
        }
        "手机锁码（PIN / NV 82）将恢复为默认值 1234。" => {
            "The phone lock code (PIN / NV 82) will be reset to 1234."
        }
        "将锁值写为 {value}，控制位 {control}。写入后锁会绑定到该值，请确认卡片匹配。" => {
            "Write lock value {value} with control bit {control}. The lock will bind to this value; verify the card matches."
        }
        "通过 0xE1 服务 + auth 4000 写入 NV 10481，control = {control}。此操作会切换维护模式位。" => {
            "Write NV 10481 through service 0xE1 + auth 4000 with control = {control}. This toggles the maintenance mode bit."
        }
        "重置 PIN" => "Reset PIN",
        "写入" => "Write",
        "应用" => "Apply",

        // ------------------------------------------------------------------
        // About
        // ------------------------------------------------------------------
        "版本 {version}" => "Version {version}",
        "基于 egui + eframe 的 Material 3 前端。协议、Type2Auth 算法与 JSON 结构均复用 Rust 后端，行为与 auRokku.py 保持一致。" => {
            "A Material 3 front-end built with egui + eframe. The protocol, Type2Auth algorithm and JSON structure reuse the Rust backend and match auRokku.py."
        }
        "跨平台：Linux（Wayland / X11）、Windows 与 macOS。USB 访问在 Linux 上通常需要 udev 规则或 root 权限。" => {
            "Cross-platform: Linux (Wayland / X11), Windows and macOS. USB access on Linux usually requires a udev rule or root privileges."
        }
        "Linux 启动时会通过 pkexec（polkit）请求 root 权限；使用 --no-polkit 或 AUROKU_GUI_NO_POLKIT=1 可跳过。认证取消或 polkit 不可用时会以普通用户继续运行。" => {
            "On Linux, the app requests root via pkexec (polkit) at startup; use --no-polkit or AUROKU_GUI_NO_POLKIT=1 to skip. If authentication is cancelled or polkit is unavailable, it continues as a normal user."
        }

        // ------------------------------------------------------------------
        // Logs / notifications / validation
        // ------------------------------------------------------------------
        "界面已启动，正在扫描 USB 总线" => "UI started; scanning the USB bus",
        "开始扫描 USB 设备" => "Scanning USB devices",
        "扫描完成，发现 {n} 个 USB 设备" => "Scan complete: {n} USB device(s) found",
        "扫描失败：{message}" => "Scan failed: {message}",
        "{label} 完成：{status}" => "{label} finished: {status}",
        "结果为失败" => "Failed",
        "{label}：成功" => "{label}: success",
        "{label}：结果为失败" => "{label}: operation reported failure",
        "{label} 错误（{kind}）：{message}" => "{label} error ({kind}): {message}",
        "已选择 {name}（{id}）" => "Selected {name} ({id})",
        "EP OUT 必须是 OUT 端点（bit 7 = 0，例如 04）" => {
            "EP OUT must be an OUT endpoint (bit 7 = 0, e.g. 04)"
        }
        "EP IN 必须是 IN 端点（bit 7 = 1，例如 84）" => {
            "EP IN must be an IN endpoint (bit 7 = 1, e.g. 84)"
        }
        "锁值需要 8 位十六进制字符（4 字节，例如 80e48ed2）" => {
            "The lock value must be 8 hex characters (4 bytes, e.g. 80e48ed2)"
        }
        "探测超时必须大于 0 秒" => "Probe timeout must be greater than 0 seconds",
        "VID / PID 格式无效" => "Invalid VID / PID format",
        "无效的十六进制值：{text}" => "Invalid hex value: {text}",
        "无效的十六进制字节：{text}" => "Invalid hex byte: {text}",
        "无法定位可执行文件：{error}；将以普通用户运行" => {
            "Unable to locate the executable: {error}; continuing as a normal user"
        }
        "可执行文件路径无效；将以普通用户运行" => {
            "The executable path is invalid; continuing as a normal user"
        }
        "可执行文件对任意用户可写，拒绝提权；请修复文件权限后重试" => {
            "The executable is world-writable; refusing elevation. Fix the file permissions and retry."
        }
        "未找到 pkexec（polkit），将以普通用户运行" => {
            "pkexec (polkit) not found; continuing as a normal user"
        }
        "正在通过 polkit 请求 root 权限…" => "Requesting root via polkit…",
        "polkit 认证未完成，将以普通用户继续运行（USB 访问可能受限）" => {
            "Polkit authentication did not complete; continuing as a normal user (USB access may be limited)"
        }
        "无法启动 pkexec：{error}；将以普通用户运行" => {
            "Unable to start pkexec: {error}; continuing as a normal user"
        }

        _ => text,
    }
}

/// Translate a template and substitute `{key}` placeholders.
pub fn tf(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = t(template).to_owned();
    for (key, value) in args {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_ui_strings_and_placeholders() {
        set(Language::En);

        assert_eq!(t("概览"), "Overview");
        assert_eq!(t("读取综合状态"), "Read full status");
        assert_eq!(
            tf("扫描完成，发现 {n} 个 USB 设备", &[("n", "2")],),
            "Scan complete: 2 USB device(s) found"
        );
        assert_eq!(
            tf("已锁定 ({value})", &[("value", "80e48ed2")]),
            "Locked (80e48ed2)"
        );
        assert_eq!(t("unknown technical string"), "unknown technical string");

        set(Language::ZhCn);
        assert_eq!(t("概览"), "概览");
        assert_eq!(
            tf("扫描完成，发现 {n} 个 USB 设备", &[("n", "2")]),
            "扫描完成，发现 2 个 USB 设备"
        );
    }
}
