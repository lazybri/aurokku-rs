#pragma once

#include <QJsonObject>
#include <QJsonValue>
#include <QString>

/// Render a JSON value the way the Rust/CLI presentation does
/// (`None`, `True`/`False`, numbers without trailing `.0`).
QString pyValue(const QJsonValue &value);

/// Read `key` as text; missing values become an empty string.
QString jsonText(const QJsonObject &object, const QString &key);

/// Read a nested object; returns an empty object when absent or not an object.
QJsonObject jsonObject(const QJsonObject &object, const QString &key);
