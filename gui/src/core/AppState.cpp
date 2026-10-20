#include "AppState.h"

#include <cmath>

#include "ui/I18n.h"

namespace {

QString hexDigits(QString text)
{
    text = text.trimmed();
    if (text.startsWith(QLatin1String("0x"), Qt::CaseInsensitive))
        text = text.mid(2);
    return text;
}

} // namespace

DeviceInfo DeviceInfo::fromJson(const QJsonValue &value)
{
    const QJsonObject object = value.toObject();
    DeviceInfo device;
    device.vid = static_cast<quint16>(object.value(QStringLiteral("vid")).toInt());
    device.pid = static_cast<quint16>(object.value(QStringLiteral("pid")).toInt());
    device.bus = object.value(QStringLiteral("bus")).toInt();
    device.address = object.value(QStringLiteral("address")).toInt();
    device.manufacturer = object.value(QStringLiteral("manufacturer")).toString();
    device.product = object.value(QStringLiteral("product")).toString();
    device.serial = object.value(QStringLiteral("serial")).toString();
    device.id = object.value(QStringLiteral("id")).toString();
    device.display = object.value(QStringLiteral("display")).toString();
    if (device.id.isEmpty()) {
        device.id = QStringLiteral("%1:%2")
                        .arg(device.vid, 4, 16, QLatin1Char('0'))
                        .arg(device.pid, 4, 16, QLatin1Char('0'));
    }
    if (device.display.isEmpty())
        device.display = device.id;
    return device;
}

void AppState::appendLog(const QString &text)
{
    LogLine line;
    line.at = elapsed.isValid() ? elapsed.elapsed() / 1000.0 : 0.0;
    line.text = text;
    logs.append(line);
    if (logs.size() > 800)
        logs.remove(0, logs.size() - 800);
}

bool parseHexU16(const QString &text, quint16 *out, QString *error)
{
    bool ok = false;
    const uint value = hexDigits(text).toUInt(&ok, 16);
    if (!ok || value > 0xFFFF) {
        if (error)
            *error = I18n::tf(QStringLiteral("无效的十六进制值：{text}"),
                              {{QStringLiteral("text"), text}});
        return false;
    }
    if (out)
        *out = static_cast<quint16>(value);
    return true;
}

bool parseHexU8(const QString &text, quint8 *out, QString *error)
{
    bool ok = false;
    const uint value = hexDigits(text).toUInt(&ok, 16);
    if (!ok || value > 0xFF) {
        if (error)
            *error = I18n::tf(QStringLiteral("无效的十六进制字节：{text}"),
                              {{QStringLiteral("text"), text}});
        return false;
    }
    if (out)
        *out = static_cast<quint8>(value);
    return true;
}

RequestResult buildRequest(const AppState &state, const QJsonObject &task)
{
    RequestResult result;

    quint16 vid = 0;
    quint16 pid = 0;
    if (!parseHexU16(state.vidText, &vid, &result.error))
        return result;
    if (!parseHexU16(state.pidText, &pid, &result.error))
        return result;

    quint8 epOut = 0;
    quint8 epIn = 0;
    if (!parseHexU8(state.epOutText, &epOut, &result.error))
        return result;
    if (!parseHexU8(state.epInText, &epIn, &result.error))
        return result;

    if ((epOut & 0x80) != 0) {
        result.error = I18n::t(QStringLiteral("EP OUT 必须是 OUT 端点（bit 7 = 0，例如 04）"));
        return result;
    }
    if ((epIn & 0x80) == 0) {
        result.error = I18n::t(QStringLiteral("EP IN 必须是 IN 端点（bit 7 = 1，例如 84）"));
        return result;
    }

    QJsonObject request;
    request.insert(QStringLiteral("vid"), vid);
    request.insert(QStringLiteral("pid"), pid);

    const QString sn = state.snText.trimmed();
    request.insert(QStringLiteral("sn"), sn.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(sn));

    const QString interfaceText = state.interfaceText.trimmed();
    if (interfaceText.isEmpty()) {
        request.insert(QStringLiteral("interface"), QJsonValue(QJsonValue::Null));
    } else {
        quint8 interfaceNumber = 0;
        if (!parseHexU8(interfaceText, &interfaceNumber, &result.error))
            return result;
        request.insert(QStringLiteral("interface"), static_cast<int>(interfaceNumber));
    }

    request.insert(QStringLiteral("ep_out"), static_cast<int>(epOut));
    request.insert(QStringLiteral("ep_in"), static_cast<int>(epIn));
    request.insert(QStringLiteral("verbose"), state.verbose);
    request.insert(QStringLiteral("type2"), state.type2);
    request.insert(QStringLiteral("task"), task);

    result.ok = true;
    result.request = request;
    return result;
}
