#include "I18n.h"

#include <QHash>
#include <QLocale>

namespace {

I18n::Language g_language = I18n::Language::ZhCn;

const QHash<QString, QString> &english()
{
    static const QHash<QString, QString> table = [] {
        QHash<QString, QString> t;

        // Navigation / chrome -------------------------------------------------
        t[QStringLiteral("概览")] = QStringLiteral("Overview");
        t[QStringLiteral("UIM 锁")] = QStringLiteral("UIM Lock");
        t[QStringLiteral("探测")] = QStringLiteral("Probe");
        t[QStringLiteral("批量")] = QStringLiteral("Batch");
        t[QStringLiteral("会话")] = QStringLiteral("Session");
        t[QStringLiteral("控制台")] = QStringLiteral("Console");
        t[QStringLiteral("关于")] = QStringLiteral("About");
        t[QStringLiteral("关于 auRokku")] = QStringLiteral("About auRokku");
        t[QStringLiteral("主题")] = QStringLiteral("Theme");
        t[QStringLiteral("界面语言")] = QStringLiteral("Interface language");
        t[QStringLiteral("简体中文")] = QStringLiteral("简体中文");
        t[QStringLiteral("English")] = QStringLiteral("English");
        t[QStringLiteral("切换浅色 / 深色主题")] = QStringLiteral("Toggle light / dark theme");
        t[QStringLiteral("该操作会通过 USB 与设备通信，设备可能重启")] =
            QStringLiteral("This operation talks to the device over USB; the device may restart.");
        t[QStringLiteral("SERI / Type2Auth 设备工具")] = QStringLiteral("SERI / Type2Auth device tool");
        t[QStringLiteral("执行中")] = QStringLiteral("Running");
        t[QStringLiteral("错误")] = QStringLiteral("Error");
        t[QStringLiteral("就绪")] = QStringLiteral("Ready");
        t[QStringLiteral("失败")] = QStringLiteral("Failed");
        t[QStringLiteral("待命")] = QStringLiteral("Idle");
        t[QStringLiteral("扫描中")] = QStringLiteral("Scanning");
        t[QStringLiteral("等待操作")] = QStringLiteral("Waiting for an operation");
        t[QStringLiteral("auRokku v{version} · 基于Yuu所提供的代码二次开发 ©lazybri")] =
            QStringLiteral("auRokku v{version} · Based on code by Yuu, further developed ©lazybri");
        t[QStringLiteral("界面语言已切换为 {lang}")] =
            QStringLiteral("Interface language: {lang}");
        t[QStringLiteral("当前操作不可用")] = QStringLiteral("This action is currently unavailable");
        t[QStringLiteral("未找到 CJK 字体，无法显示中文界面")] =
            QStringLiteral("No CJK font found; the Chinese UI is unavailable");
        t[QStringLiteral("取消")] = QStringLiteral("Cancel");
        t[QStringLiteral("关闭")] = QStringLiteral("Close");
        t[QStringLiteral("确定")] = QStringLiteral("OK");
        t[QStringLiteral("后端版本")] = QStringLiteral("Backend version");
        t[QStringLiteral("Qt 版本")] = QStringLiteral("Qt version");
        t[QStringLiteral("后端 FFI 已连接")] = QStringLiteral("Rust FFI backend connected");
        t[QStringLiteral("请先扫描并选择 USB 设备")] =
            QStringLiteral("Scan for and select a USB device first");
        t[QStringLiteral("所选 USB 设备的临时访问权限已失效，请重新选择设备")] =
            QStringLiteral("Temporary access to the selected USB device expired; select it again");
        t[QStringLiteral("VID / PID 与所选 USB 设备不匹配")] =
            QStringLiteral("VID / PID does not match the selected USB device");
        t[QStringLiteral("USB ACL 清理失败：{error}")] =
            QStringLiteral("Failed to restore USB ACL: {error}");
        t[QStringLiteral("USB ACL 清理失败")] = QStringLiteral("USB ACL cleanup failed");
        t[QStringLiteral("无法授予所选 USB 设备访问权限：{error}")] =
            QStringLiteral("Could not grant access to the selected USB device: {error}");
        t[QStringLiteral("选择 USB 设备需要 pkexec 和 polkit 授权；GUI 本身不会以 root 运行。")] =
            QStringLiteral("Selecting a USB device requires pkexec/polkit authorization; the GUI itself never runs as root.");

        // Overview ------------------------------------------------------------
        t[QStringLiteral("连接 au / SERI 设备，读取锁状态、PIN 与 SIM 信息")] =
            QStringLiteral("Connect to an au / SERI device and read lock state, PIN and SIM information");
        t[QStringLiteral("扫描后选择设备将请求授权，并仅为当前 USB 设备临时添加读写 ACL；更换设备、重新扫描或退出软件时会恢复原规则。")] =
            QStringLiteral("Selecting a scanned device requests authorization and adds a temporary read/write ACL only to that device. The original ACL is restored when changing devices, rescanning or exiting.");
        t[QStringLiteral("请选择 USB 设备")] = QStringLiteral("Select a USB device");
        t[QStringLiteral("未发现设备（请扫描 USB 设备）")] =
            QStringLiteral("No devices found (scan for USB devices)");
        t[QStringLiteral("选择设备")] = QStringLiteral("Select device");
        t[QStringLiteral("设备连接")] = QStringLiteral("Device connection");
        t[QStringLiteral("扫描 USB 总线并选择目标设备")] =
            QStringLiteral("Scan the USB bus and select a target device");
        t[QStringLiteral("扫描中…")] = QStringLiteral("Scanning…");
        t[QStringLiteral("扫描全部")] = QStringLiteral("Scan all");
        t[QStringLiteral("仅筛选 VID/PID")] = QStringLiteral("Filter VID/PID");
        t[QStringLiteral("SN（可选）")] = QStringLiteral("SN (optional)");
        t[QStringLiteral("SKYKA077394 或 16 位 hex")] = QStringLiteral("SKYKA077394 or 16 hex digits");
        t[QStringLiteral("接口号（可选）")] = QStringLiteral("Interface (optional)");
        t[QStringLiteral("自动")] = QStringLiteral("Auto");
        t[QStringLiteral("Type2Auth 52/53 认证")] = QStringLiteral("Type2Auth 52/53 authentication");
        t[QStringLiteral("详细 USB 日志")] = QStringLiteral("Verbose USB log");
        t[QStringLiteral("SN 留空时读取设备 iSerialNumber；若设备不提供序列号，请手动填写 11 位 SN 或 16 位十六进制。")] =
            QStringLiteral("When SN is empty the device iSerialNumber is used. If the device has no serial, enter an 11-character SN or 16 hex digits manually.");
        t[QStringLiteral("快捷操作")] = QStringLiteral("Quick actions");
        t[QStringLiteral("所有 USB 操作都在后台线程执行，界面不会卡住")] =
            QStringLiteral("All USB operations run on a background thread, so the UI stays responsive");
        t[QStringLiteral("读取综合状态")] = QStringLiteral("Read full status");
        t[QStringLiteral("USB 布局探测")] = QStringLiteral("USB layout probe");
        t[QStringLiteral("退出服务 (0xD8)")] = QStringLiteral("Exit service (0xD8)");
        t[QStringLiteral("打开日志控制台")] = QStringLiteral("Open log console");
        t[QStringLiteral("正在执行：{label}")] = QStringLiteral("Running: {label}");
        t[QStringLiteral("最近结果")] = QStringLiteral("Latest result");
        t[QStringLiteral("最近一次操作的结构化摘要")] =
            QStringLiteral("Structured summary of the latest operation");
        t[QStringLiteral("成功")] = QStringLiteral("Success");
        t[QStringLiteral("查看完整 JSON")] = QStringLiteral("View full JSON");
        t[QStringLiteral("还没有结果")] = QStringLiteral("No result yet");
        t[QStringLiteral("运行一次状态读取或探测后，结果会显示在这里")] =
            QStringLiteral("Run a status read or probe and the result will appear here");
        t[QStringLiteral("设备 SN")] = QStringLiteral("Device SN");
        t[QStringLiteral("手机 PIN")] = QStringLiteral("Phone PIN");
        t[QStringLiteral("Base 版本 (raw)")] = QStringLiteral("Base version (raw)");
        t[QStringLiteral("已解锁 (ffffffff)")] = QStringLiteral("Unlocked (ffffffff)");
        t[QStringLiteral("已锁定 ({value})")] = QStringLiteral("Locked ({value})");

        // Generic result keys / values ---------------------------------------
        t[QStringLiteral("UIM 锁值")] = QStringLiteral("UIM lock value");
        t[QStringLiteral("Control bit")] = QStringLiteral("Control bit");
        t[QStringLiteral("控制位")] = QStringLiteral("Control bit");
        t[QStringLiteral("解码数据")] = QStringLiteral("Decoded data");
        t[QStringLiteral("原始数据")] = QStringLiteral("Raw data");
        t[QStringLiteral("应答")] = QStringLiteral("Reply");
        t[QStringLiteral("请求")] = QStringLiteral("Request");
        t[QStringLiteral("响应")] = QStringLiteral("Response");
        t[QStringLiteral("维护位")] = QStringLiteral("Maintenance bit");
        t[QStringLiteral("退出方式")] = QStringLiteral("Exit method");
        t[QStringLiteral("写入校验")] = QStringLiteral("Write verification");
        t[QStringLiteral("通过")] = QStringLiteral("Passed");
        t[QStringLiteral("未通过")] = QStringLiteral("Failed");
        t[QStringLiteral("命令接受")] = QStringLiteral("Command accepted");
        t[QStringLiteral("是")] = QStringLiteral("Yes");
        t[QStringLiteral("否")] = QStringLiteral("No");
        t[QStringLiteral("该操作没有可展开的摘要字段，请查看完整 JSON。")] =
            QStringLiteral("This operation has no expandable summary fields; view the full JSON.");

        // UIM lock ------------------------------------------------------------
        t[QStringLiteral("读取、写入或清除 SIM 锁绑定（0x54 / 0x72 / 0x57）")] =
            QStringLiteral("Read, write or clear the SIM lock binding (0x54 / 0x72 / 0x57)");
        t[QStringLiteral("写入或清除锁会修改 SIM 绑定状态。执行前请确认设备与卡片匹配，避免写入错误绑定值。")] =
            QStringLiteral("Writing or clearing the lock modifies the SIM binding state. Verify the device and card before continuing to avoid writing an incorrect binding value.");
        t[QStringLiteral("读取锁状态")] = QStringLiteral("Read lock state");
        t[QStringLiteral("进入 UIMLK 会话并读取 control 位与绑定值")] =
            QStringLiteral("Enter a UIMLK session and read the control bit and binding value");
        t[QStringLiteral("读取 UIM 锁")] = QStringLiteral("Read UIM lock");
        t[QStringLiteral("写入绑定值")] = QStringLiteral("Write binding value");
        t[QStringLiteral("0x72：写 4 字节锁值并校验回读")] =
            QStringLiteral("0x72: write a 4-byte lock value and verify it by reading back");
        t[QStringLiteral("锁值（8 位 hex）")] = QStringLiteral("Lock value (8 hex digits)");
        t[QStringLiteral("清除绑定")] = QStringLiteral("Clear binding");
        t[QStringLiteral("0x57：把锁值写为 ffffffff（解除绑定）")] =
            QStringLiteral("0x57: write ffffffff to the lock value (remove binding)");
        t[QStringLiteral("清除 UIM 锁绑定")] = QStringLiteral("Clear UIM lock binding");
        t[QStringLiteral("写入 UIM 锁绑定")] = QStringLiteral("Write UIM lock binding");
        t[QStringLiteral("设置维护模式位")] = QStringLiteral("Set maintenance mode bit");
        t[QStringLiteral("命令接受后会回读锁值并校验是否为 ffffffff。")] =
            QStringLiteral("After the command is accepted, the lock value is read back and verified as ffffffff.");
        t[QStringLiteral("维护模式位 (NV 10481)")] = QStringLiteral("Maintenance mode bit (NV 10481)");
        t[QStringLiteral("0xE1 服务 + auth 4000 认证后写入")] =
            QStringLiteral("Written after service 0xE1 + auth 4000");
        t[QStringLiteral("应用维护位")] = QStringLiteral("Apply maintenance bit");
        t[QStringLiteral("需要有效的 SN / Type2Auth 材料；认证失败时会返回 0x53 校验错误。")] =
            QStringLiteral("A valid SN / Type2Auth material is required; authentication failure returns a 0x53 verification error.");
        t[QStringLiteral("已解锁 / 已清除")] = QStringLiteral("Unlocked / cleared");
        t[QStringLiteral("已锁定")] = QStringLiteral("Locked");
        t[QStringLiteral("锁值")] = QStringLiteral("Lock value");
        t[QStringLiteral("原始应答")] = QStringLiteral("Raw reply");
        t[QStringLiteral("写入前")] = QStringLiteral("Before write");
        t[QStringLiteral("写入后")] = QStringLiteral("After write");
        t[QStringLiteral("回读校验")] = QStringLiteral("Read-back verification");

        // PIN / ICCID ---------------------------------------------------------
        t[QStringLiteral("读取或重置手机锁码（NV 82），读取 ICCID record 0x59")] =
            QStringLiteral("Read or reset the phone lock code (NV 82) and read ICCID record 0x59");
        t[QStringLiteral("读取手机锁码")] = QStringLiteral("Read phone lock code");
        t[QStringLiteral("SERI 0xDC 0x00，按 0xAA 终止符解析")] =
            QStringLiteral("SERI 0xDC 0x00, parsed by the 0xAA terminator");
        t[QStringLiteral("读取 PIN")] = QStringLiteral("Read PIN");
        t[QStringLiteral("手机锁码")] = QStringLiteral("Phone lock code");
        t[QStringLiteral("重置手机锁码")] = QStringLiteral("Reset phone lock code");
        t[QStringLiteral("SERI 0xDC 0x01，恢复默认值 1234 并回读校验")] =
            QStringLiteral("SERI 0xDC 0x01: restore the default 1234 and verify by reading back");
        t[QStringLiteral("重置 PIN 为 1234")] = QStringLiteral("Reset PIN to 1234");
        t[QStringLiteral("回读 PIN")] = QStringLiteral("Read-back PIN");
        t[QStringLiteral("1234 校验")] = QStringLiteral("1234 verification");
        t[QStringLiteral("ICCID 记录")] = QStringLiteral("ICCID record");
        t[QStringLiteral("进入 UIMLK 会话读取 record 0x59")] =
            QStringLiteral("Enter a UIMLK session and read record 0x59");
        t[QStringLiteral("读取 ICCID")] = QStringLiteral("Read ICCID");
        t[QStringLiteral("解码 ICCID")] = QStringLiteral("Decoded ICCID");

        // Probe ---------------------------------------------------------------
        t[QStringLiteral("枚举 USB 布局，或主动探测 SERI / SCDP 服务")] =
            QStringLiteral("Enumerate the USB layout or actively probe the SERI / SCDP services");
        t[QStringLiteral("探测模式")] = QStringLiteral("Probe mode");
        t[QStringLiteral("list 只枚举配置；seri / scdp 会实际收发 USB 数据")] =
            QStringLiteral("list only enumerates configurations; seri / scdp actually send and receive USB data");
        t[QStringLiteral("布局 list")] = QStringLiteral("Layout list");
        t[QStringLiteral("SERI 回声")] = QStringLiteral("SERI echo");
        t[QStringLiteral("探测超时（秒）")] = QStringLiteral("Probe timeout (seconds)");
        t[QStringLiteral("开始探测")] = QStringLiteral("Start probe");
        t[QStringLiteral("SERI 探测会发送 1C 05；非 0FCE 设备在成功后自动发送 D8 清理。")] =
            QStringLiteral("SERI probe sends 1C 05; non-0FCE devices automatically send D8 cleanup on success.");
        t[QStringLiteral("USB 布局")] = QStringLiteral("USB layout");
        t[QStringLiteral("配置 / 接口 / 端点与候选 bulk 端点对")] =
            QStringLiteral("Configurations / interfaces / endpoints and candidate bulk endpoint pairs");
        t[QStringLiteral("尚无探测结果")] = QStringLiteral("No probe result yet");
        t[QStringLiteral("运行布局探测后，这里会显示 USB 配置树与端点信息")] =
            QStringLiteral("Run a layout probe to see the USB configuration tree and endpoint details here");
        t[QStringLiteral("SERI 回声匹配")] = QStringLiteral("SERI echo matched");
        t[QStringLiteral("设备有响应")] = QStringLiteral("Device responded");
        t[QStringLiteral("选中接口")] = QStringLiteral("Selected interface");
        t[QStringLiteral("<超时>")] = QStringLiteral("<timeout>");
        t[QStringLiteral("清理命令")] = QStringLiteral("Cleanup command");
        t[QStringLiteral("设备 {vid}:{pid}")] = QStringLiteral("Device {vid}:{pid}");
        t[QStringLiteral("配置 {value}")] = QStringLiteral("Config {value}");
        t[QStringLiteral("活动")] = QStringLiteral("Active");
        t[QStringLiteral("接口 {number} · alt {alt}")] = QStringLiteral("Interface {number} · alt {alt}");
        t[QStringLiteral("未发现 bulk 端点对")] = QStringLiteral("No bulk endpoint pairs found");
        t[QStringLiteral("候选 bulk 端点对")] = QStringLiteral("Candidate bulk endpoint pairs");
        t[QStringLiteral("SERI 回声探测")] = QStringLiteral("SERI echo probe");
        t[QStringLiteral("SCDP 探测")] = QStringLiteral("SCDP probe");

        // Batch ---------------------------------------------------------------
        t[QStringLiteral("批量查询")] = QStringLiteral("Batch query");
        t[QStringLiteral("在单个会话内连续执行多条查询，避免反复进入 / 退出服务")] =
            QStringLiteral("Run multiple queries in one session to avoid repeated service entry/exit");
        t[QStringLiteral("批量配置")] = QStringLiteral("Batch profile");
        t[QStringLiteral("basic：版本 + 锁码；uimlk：锁状态 + record59")] =
            QStringLiteral("basic: version + lock code; uimlk: lock state + record59");
        t[QStringLiteral("基础 basic")] = QStringLiteral("Basic");
        t[QStringLiteral("UIMLK uimlk")] = QStringLiteral("UIMLK");
        t[QStringLiteral("执行批量查询")] = QStringLiteral("Run batch query");
        t[QStringLiteral("uimlk profile 需要先完成 UIMLK 认证，SN 必须可用。")] =
            QStringLiteral("The uimlk profile requires UIMLK authentication and a usable SN.");
        t[QStringLiteral("查询结果")] = QStringLiteral("Query results");
        t[QStringLiteral("每条查询的原始字段与解码值")] =
            QStringLiteral("Raw fields and decoded values for each query");
        t[QStringLiteral("查询：{query}")] = QStringLiteral("Query: {query}");
        t[QStringLiteral("尚无批量结果")] = QStringLiteral("No batch result yet");
        t[QStringLiteral("选择 profile 并执行查询后，这里会列出每条结果")] =
            QStringLiteral("Select a profile and run the query to list each result here");

        // Session -------------------------------------------------------------
        t[QStringLiteral("会话与服务")] = QStringLiteral("Session & services");
        t[QStringLiteral("退出维护 / UIM 服务，让设备回到正常模式")] =
            QStringLiteral("Exit maintenance / UIM services and return the device to normal mode");
        t[QStringLiteral("退出基础服务")] = QStringLiteral("Exit base service");
        t[QStringLiteral("发送 0xD8，恢复手机键盘 / 正常界面")] =
            QStringLiteral("Send 0xD8 to restore the phone keypad / normal UI");
        t[QStringLiteral("对 0FCE 设备不会发送 0xD8（与 Python 版一致）。")] =
            QStringLiteral("0xD8 is not sent to 0FCE devices (same as the Python version).");
        t[QStringLiteral("退出 UIMLK 子服务")] = QStringLiteral("Exit UIMLK sub-service");
        t[QStringLiteral("重新进入 UIMLK 并发送 0x56，设备可能重启")] =
            QStringLiteral("Re-enter UIMLK and send 0x56; the device may reboot");
        t[QStringLiteral("退出 UIMLK (0x56)")] = QStringLiteral("Exit UIMLK (0x56)");
        t[QStringLiteral("返回结果中的 exit 字段会记录 acknowledged / reply / may_reboot。")] =
            QStringLiteral("The exit field records acknowledged / reply / may_reboot.");
        t[QStringLiteral("退出服务")] = QStringLiteral("Exit service");
        t[QStringLiteral("退出 UIMLK")] = QStringLiteral("Exit UIMLK");
        t[QStringLiteral("会话详情")] = QStringLiteral("Session details");
        t[QStringLiteral("退出应答与异步事件")] = QStringLiteral("Exit reply and asynchronous events");
        t[QStringLiteral("退出已确认")] = QStringLiteral("Exit acknowledged");
        t[QStringLiteral("退出未确认")] = QStringLiteral("Exit not acknowledged");
        t[QStringLiteral("可能重启")] = QStringLiteral("May reboot");
        t[QStringLiteral("本次操作没有产生服务退出记录。")] =
            QStringLiteral("This operation produced no service exit record.");
        t[QStringLiteral("异步事件")] = QStringLiteral("Async events");
        t[QStringLiteral("还没有会话记录")] = QStringLiteral("No session record yet");
        t[QStringLiteral("执行退出服务后，应答信息会显示在这里")] =
            QStringLiteral("Run an exit service operation and the reply will appear here");

        // Console -------------------------------------------------------------
        t[QStringLiteral("USB 收发日志与最近一次操作的完整 JSON")] =
            QStringLiteral("USB traffic log and the full JSON of the latest operation");
        t[QStringLiteral("自动滚动")] = QStringLiteral("Auto-scroll");
        t[QStringLiteral("清空日志")] = QStringLiteral("Clear log");
        t[QStringLiteral("{count} 条")] = QStringLiteral("{count} entries");
        t[QStringLiteral("暂无日志。启用“详细 USB 日志”可看到 TX / RX 帧。")] =
            QStringLiteral("No logs yet. Enable “Verbose USB log” to see TX / RX frames.");
        t[QStringLiteral("日志已清空")] = QStringLiteral("Log cleared");
        t[QStringLiteral("完整 JSON")] = QStringLiteral("Full JSON");
        t[QStringLiteral("最近一次操作")] = QStringLiteral("Latest operation");
        t[QStringLiteral("复制")] = QStringLiteral("Copy");
        t[QStringLiteral("没有 JSON 结果")] = QStringLiteral("No JSON result");
        t[QStringLiteral("执行一次操作后，完整协议结果会显示在这里")] =
            QStringLiteral("Run an operation and the full protocol result will appear here");
        t[QStringLiteral("JSON 已复制到剪贴板")] = QStringLiteral("JSON copied to clipboard");

        // Confirm dialogs -----------------------------------------------------
        t[QStringLiteral("清除 UIM 锁绑定？")] = QStringLiteral("Clear UIM lock binding?");
        t[QStringLiteral("重置手机锁码？")] = QStringLiteral("Reset phone lock code?");
        t[QStringLiteral("写入 UIM 锁绑定？")] = QStringLiteral("Write UIM lock binding?");
        t[QStringLiteral("设置维护模式位？")] = QStringLiteral("Set maintenance mode bit?");
        t[QStringLiteral("此操作会把锁值写为 ffffffff 并解除 SIM 绑定。请确认设备与卡片状态后再继续。")] =
            QStringLiteral("This writes ffffffff to the lock value and removes the SIM binding. Verify the device and card state before continuing.");
        t[QStringLiteral("手机锁码（PIN / NV 82）将恢复为默认值 1234。")] =
            QStringLiteral("The phone lock code (PIN / NV 82) will be reset to 1234.");
        t[QStringLiteral("将锁值写为 {value}，控制位 {control}。写入后锁会绑定到该值，请确认卡片匹配。")] =
            QStringLiteral("Write lock value {value} with control bit {control}. The lock will bind to this value; verify the card matches.");
        t[QStringLiteral("通过 0xE1 服务 + auth 4000 写入 NV 10481，control = {control}。此操作会切换维护模式位。")] =
            QStringLiteral("Write NV 10481 through service 0xE1 + auth 4000 with control = {control}. This toggles the maintenance mode bit.");
        t[QStringLiteral("重置 PIN")] = QStringLiteral("Reset PIN");
        t[QStringLiteral("写入")] = QStringLiteral("Write");
        t[QStringLiteral("应用")] = QStringLiteral("Apply");
        t[QStringLiteral("清除绑定")] = QStringLiteral("Clear binding");

        // About ---------------------------------------------------------------
        t[QStringLiteral("版本 {version}")] = QStringLiteral("Version {version}");
        t[QStringLiteral("基于 Qt 6 Widgets 的前端，通过 C ABI 直接调用 Rust 后端。协议、Type2Auth 算法与 JSON 结构均与 auRokku.py 保持一致。")] =
            QStringLiteral("A Qt 6 Widgets front-end that calls the Rust backend through a C ABI. The protocol, Type2Auth algorithm and JSON structure match auRokku.py.");
        t[QStringLiteral("跨平台：Linux、Windows 与 macOS。Linux 通过 ACL 为所选 USB 设备临时授予读写权限。")] =
            QStringLiteral("Cross-platform: Linux, Windows and macOS. Linux temporarily grants USB read/write access to the selected device through ACLs.");
        t[QStringLiteral("Linux GUI 始终以普通用户运行。修改 ACL 时只通过 pkexec 执行 setfacl，不会提权重启整个 GUI；需要安装 acl 和 polkit。")] =
            QStringLiteral("The Linux GUI always runs as the current user. It uses pkexec only to run setfacl for ACL changes, never to elevate the GUI; acl and polkit are required.");
        t[QStringLiteral("所有 USB 操作均在后台线程执行。")] =
            QStringLiteral("All USB operations run on a background thread.");

        // Logs / notifications / validation -----------------------------------
        t[QStringLiteral("界面已启动，正在扫描 USB 总线")] =
            QStringLiteral("UI started; scanning the USB bus");
        t[QStringLiteral("开始扫描 USB 设备")] = QStringLiteral("Scanning USB devices");
        t[QStringLiteral("扫描完成，发现 {n} 个 USB 设备")] =
            QStringLiteral("Scan complete: {n} USB device(s) found");
        t[QStringLiteral("扫描失败：{message}")] = QStringLiteral("Scan failed: {message}");
        t[QStringLiteral("{label} 完成：{status}")] = QStringLiteral("{label} finished: {status}");
        t[QStringLiteral("结果为失败")] = QStringLiteral("Failed");
        t[QStringLiteral("{label}：成功")] = QStringLiteral("{label}: success");
        t[QStringLiteral("{label}：结果为失败")] =
            QStringLiteral("{label}: operation reported failure");
        t[QStringLiteral("{label} 错误（{kind}）：{message}")] =
            QStringLiteral("{label} error ({kind}): {message}");
        t[QStringLiteral("已选择 {name}（{id}）")] = QStringLiteral("Selected {name} ({id})");
        t[QStringLiteral("EP OUT 必须是 OUT 端点（bit 7 = 0，例如 04）")] =
            QStringLiteral("EP OUT must be an OUT endpoint (bit 7 = 0, e.g. 04)");
        t[QStringLiteral("EP IN 必须是 IN 端点（bit 7 = 1，例如 84）")] =
            QStringLiteral("EP IN must be an IN endpoint (bit 7 = 1, e.g. 84)");
        t[QStringLiteral("锁值需要 8 位十六进制字符（4 字节，例如 80e48ed2）")] =
            QStringLiteral("The lock value must be 8 hex characters (4 bytes, e.g. 80e48ed2)");
        t[QStringLiteral("探测超时必须大于 0 秒")] =
            QStringLiteral("Probe timeout must be greater than 0 seconds");
        t[QStringLiteral("VID / PID 格式无效")] = QStringLiteral("Invalid VID / PID format");
        t[QStringLiteral("无效的十六进制值：{text}")] = QStringLiteral("Invalid hex value: {text}");
        t[QStringLiteral("无效的十六进制字节：{text}")] = QStringLiteral("Invalid hex byte: {text}");
        // WinUSB driver (ABI 2) ------------------------------------------------
        t[QStringLiteral("WinUSB 驱动")] = QStringLiteral("WinUSB driver");
        t[QStringLiteral("Windows 上设备默认由 usbser.sys 占用，需要安装 WinUSB 驱动才能访问")] =
            QStringLiteral("On Windows the device is owned by usbser.sys by default; install the WinUSB driver to access it");
        t[QStringLiteral("安装 WinUSB 驱动")] = QStringLiteral("Install WinUSB driver");
        t[QStringLiteral("恢复原驱动")] = QStringLiteral("Restore original driver");
        t[QStringLiteral("退出时自动恢复原驱动")] =
            QStringLiteral("Restore the original driver on exit");
        t[QStringLiteral("该设备当前由 usbser.sys 占用；libusb 需要 WinUSB 驱动才能访问。安装时需要确认一次 UAC。")] =
            QStringLiteral("This device is owned by usbser.sys; libusb needs the WinUSB driver to access it. Installing asks for UAC confirmation once.");
        t[QStringLiteral("正在安装 WinUSB 驱动…")] =
            QStringLiteral("Installing the WinUSB driver…");
        t[QStringLiteral("正在安装 WinUSB 驱动（需要管理员确认）…")] =
            QStringLiteral("Installing the WinUSB driver (administrator confirmation required)…");
        t[QStringLiteral("正在恢复原始驱动…")] =
            QStringLiteral("Restoring the original driver…");
        t[QStringLiteral("正在恢复原始驱动（需要管理员确认）…")] =
            QStringLiteral("Restoring the original driver (administrator confirmation required)…");
        t[QStringLiteral("已为 {target} 安装 WinUSB 驱动")] =
            QStringLiteral("Installed the WinUSB driver for {target}");
        t[QStringLiteral("WinUSB 驱动已安装，正在重试…")] =
            QStringLiteral("WinUSB driver installed; retrying…");
        t[QStringLiteral("已为 {target} 恢复原始 USB 驱动")] =
            QStringLiteral("Restored the original USB driver for {target}");
        t[QStringLiteral("已恢复原始 USB 驱动")] =
            QStringLiteral("Restored the original USB driver");
        t[QStringLiteral("驱动操作失败：{message}")] =
            QStringLiteral("Driver operation failed: {message}");
        t[QStringLiteral("驱动日志：{path}")] = QStringLiteral("Driver log: {path}");
        t[QStringLiteral("上次会话未恢复 {target} 的原始 USB 驱动，可在概览页的 WinUSB 驱动卡片中恢复")] =
            QStringLiteral("The previous session did not restore the original USB driver for {target}; restore it from the WinUSB driver card on the Overview page");
        t[QStringLiteral("正在恢复 {target} 的原始 USB 驱动…")] =
            QStringLiteral("Restoring the original USB driver for {target}…");
        t[QStringLiteral("该设备需要 WinUSB 驱动；可在概览页的 WinUSB 驱动卡片中安装。")] =
            QStringLiteral("This device needs the WinUSB driver; install it from the WinUSB driver card on the Overview page.");
        t[QStringLiteral("正在处理 WinUSB 驱动操作…")] =
            QStringLiteral("WinUSB driver operation in progress…");
        t[QStringLiteral("正在读取驱动状态…")] = QStringLiteral("Reading the driver state…");
        t[QStringLiteral("选择设备后可安装或恢复 WinUSB 驱动")] =
            QStringLiteral("Select a device to install or restore the WinUSB driver");
        t[QStringLiteral("目标设备：{target}")] = QStringLiteral("Target device: {target}");
        t[QStringLiteral("当前 Service：{service}")] = QStringLiteral("Current service: {service}");
        t[QStringLiteral("当前 Service：usbser.sys（需要替换）")] =
            QStringLiteral("Current service: usbser.sys (needs replacing)");
        t[QStringLiteral("已复制驱动日志路径")] =
            QStringLiteral("Driver log path copied");
        t[QStringLiteral("复制路径")] = QStringLiteral("Copy path");
        t[QStringLiteral("无法读取驱动状态：{message}")] =
            QStringLiteral("Could not read the driver state: {message}");
        t[QStringLiteral("驱动操作进行中")] =
            QStringLiteral("Driver operation in progress");
        t[QStringLiteral("正在安装或恢复 WinUSB 驱动，请等待操作完成后再关闭。")] =
            QStringLiteral("A WinUSB driver operation is running; wait for it to finish before closing.");
        t[QStringLiteral("恢复原始驱动失败")] =
            QStringLiteral("Failed to restore the original driver");
        t[QStringLiteral("恢复原始驱动失败：{error}")] =
            QStringLiteral("Failed to restore the original driver: {error}");
        t[QStringLiteral("是否仍然退出？")] = QStringLiteral("Quit anyway?");

        return t;
    }();
    return table;
}

} // namespace

I18n::I18n(QObject *parent)
    : QObject(parent)
{
}

I18n &I18n::instance()
{
    static I18n instance;
    return instance;
}

I18n::Language I18n::language() const
{
    return g_language;
}

void I18n::setLanguage(Language language)
{
    if (g_language == language)
        return;
    g_language = language;
    emit I18n::instance().languageChanged();
}

I18n::Language I18n::detectLanguage()
{
    return QLocale::system().name().startsWith(QLatin1String("zh")) ? Language::ZhCn : Language::En;
}

QString I18n::t(const QString &source)
{
    if (g_language == Language::ZhCn)
        return source;
    const auto it = english().constFind(source);
    return it == english().constEnd() ? source : it.value();
}

QString I18n::tf(const QString &source, const QVariantMap &arguments)
{
    QString out = t(source);
    for (auto it = arguments.begin(); it != arguments.end(); ++it) {
        out.replace(QStringLiteral("{") + it.key() + QStringLiteral("}"), it.value().toString());
    }
    return out;
}

QString I18n::nativeName() const
{
    return g_language == Language::ZhCn ? QStringLiteral("中文") : QStringLiteral("English");
}
