#include "JsonHelpers.h"

#include <QJsonArray>
#include <QStringList>
#include <cmath>

QString pyValue(const QJsonValue &value)
{
    switch (value.type()) {
    case QJsonValue::Null:
    case QJsonValue::Undefined:
        return QStringLiteral("None");
    case QJsonValue::Bool:
        return value.toBool() ? QStringLiteral("True") : QStringLiteral("False");
    case QJsonValue::Double: {
        const double number = value.toDouble();
        if (std::isfinite(number) && std::floor(number) == number && std::fabs(number) < 1e15)
            return QString::number(static_cast<qlonglong>(number));
        return QString::number(number, 'g', 15);
    }
    case QJsonValue::String:
        return value.toString();
    case QJsonValue::Array: {
        QStringList parts;
        const QJsonArray array = value.toArray();
        for (const QJsonValue &item : array)
            parts.append(pyValue(item));
        return QStringLiteral("[%1]").arg(parts.join(QStringLiteral(", ")));
    }
    case QJsonValue::Object: {
        QStringList parts;
        const QJsonObject object = value.toObject();
        for (auto it = object.begin(); it != object.end(); ++it)
            parts.append(QStringLiteral("%1: %2").arg(it.key(), pyValue(it.value())));
        return QStringLiteral("{%1}").arg(parts.join(QStringLiteral(", ")));
    }
    }
    return {};
}

QString jsonText(const QJsonObject &object, const QString &key)
{
    const QJsonValue value = object.value(key);
    if (value.isUndefined() || value.isNull())
        return {};
    if (value.isString())
        return value.toString();
    return pyValue(value);
}

QJsonObject jsonObject(const QJsonObject &object, const QString &key)
{
    return object.value(key).toObject();
}
