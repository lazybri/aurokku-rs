#include "Icons.h"

#include <QPainter>
#include <QPainterPath>
#include <QPixmap>
#include <QPolygonF>
#include <cmath>

namespace {

constexpr double kPi = 3.14159265358979323846;

void drawIcon(QPainter &painter, Icon id, const QColor &color)
{
    painter.setRenderHint(QPainter::Antialiasing, true);
    QPen pen(color, 1.9, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    painter.setPen(pen);
    painter.setBrush(Qt::NoBrush);

    switch (id) {
    case Icon::None:
        break;

    case Icon::Dashboard:
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawRoundedRect(QRectF(3, 3, 8, 8), 2, 2);
        painter.drawRoundedRect(QRectF(13, 3, 8, 5), 2, 2);
        painter.drawRoundedRect(QRectF(13, 10, 8, 11), 2, 2);
        painter.drawRoundedRect(QRectF(3, 13, 8, 8), 2, 2);
        break;

    case Icon::Lock:
        painter.drawRoundedRect(QRectF(5, 10.5, 14, 10), 3, 3);
        painter.drawPath([] {
            QPainterPath path;
            path.moveTo(8.5, 10.5);
            path.lineTo(8.5, 8);
            path.arcTo(QRectF(8.5, 4, 7, 7), 180, -180);
            path.lineTo(15.5, 10.5);
            return path;
        }());
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawEllipse(QPointF(12, 15.5), 1.4, 1.4);
        break;

    case Icon::Key:
        painter.drawEllipse(QPointF(8, 8), 4.2, 4.2);
        painter.drawLine(QPointF(11.2, 11.2), QPointF(20.5, 20.5));
        painter.drawLine(QPointF(17.2, 17.2), QPointF(20, 14.4));
        painter.drawLine(QPointF(14.6, 14.6), QPointF(17.4, 11.8));
        break;

    case Icon::Probe:
        painter.drawEllipse(QPointF(10.2, 10.2), 6, 6);
        painter.drawLine(QPointF(14.6, 14.6), QPointF(20.5, 20.5));
        break;

    case Icon::List:
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        for (int i = 0; i < 3; ++i) {
            const qreal y = 6 + i * 6;
            painter.drawEllipse(QPointF(4.5, y), 1.3, 1.3);
            painter.setPen(QPen(color, 1.9, Qt::SolidLine, Qt::RoundCap));
            painter.drawLine(QPointF(8, y), QPointF(20, y));
            painter.setPen(Qt::NoPen);
        }
        break;

    case Icon::Power:
        painter.drawArc(QRectF(4.5, 4.5, 15, 15), 55 * 16, 250 * 16);
        painter.drawLine(QPointF(12, 2.8), QPointF(12, 12));
        break;

    case Icon::Terminal:
        painter.drawRoundedRect(QRectF(2.8, 4.2, 18.4, 15.6), 3, 3);
        painter.drawPolyline(QPolygonF({QPointF(7, 9.2), QPointF(10, 12), QPointF(7, 14.8)}));
        painter.drawLine(QPointF(12.5, 15), QPointF(17, 15));
        break;

    case Icon::Usb:
        painter.drawLine(QPointF(12, 21), QPointF(12, 5));
        painter.drawPolyline(
            QPolygonF({QPointF(9, 8), QPointF(12, 4.5), QPointF(15, 8)}));
        painter.drawLine(QPointF(12, 15), QPointF(6.5, 11.5));
        painter.drawLine(QPointF(6.5, 11.5), QPointF(6.5, 8.5));
        painter.drawLine(QPointF(12, 18), QPointF(17.5, 14.5));
        painter.drawLine(QPointF(17.5, 14.5), QPointF(17.5, 11.5));
        painter.drawRect(QRectF(5, 5.6, 3, 3));
        painter.drawEllipse(QPointF(17.5, 10), 1.6, 1.6);
        painter.setBrush(color);
        painter.drawEllipse(QPointF(12, 21), 1.7, 1.7);
        break;

    case Icon::Scan: {
        const qreal s = 18;
        const qreal o = 3;
        painter.drawLine(QPointF(o, o + 5), QPointF(o, o));
        painter.drawLine(QPointF(o, o), QPointF(o + 5, o));
        painter.drawLine(QPointF(o + s - 5, o), QPointF(o + s, o));
        painter.drawLine(QPointF(o + s, o), QPointF(o + s, o + 5));
        painter.drawLine(QPointF(o, o + s - 5), QPointF(o, o + s));
        painter.drawLine(QPointF(o, o + s), QPointF(o + 5, o + s));
        painter.drawLine(QPointF(o + s - 5, o + s), QPointF(o + s, o + s));
        painter.drawLine(QPointF(o + s, o + s), QPointF(o + s, o + s - 5));
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawEllipse(QPointF(12, 12), 2.2, 2.2);
        break;
    }

    case Icon::Write: {
        QPainterPath path;
        path.moveTo(4, 20);
        path.lineTo(5.4, 14.4);
        path.lineTo(15.2, 4.6);
        path.lineTo(19.4, 8.8);
        path.lineTo(9.6, 18.6);
        path.closeSubpath();
        painter.drawPath(path);
        painter.drawLine(QPointF(13.4, 6.4), QPointF(17.6, 10.6));
        break;
    }

    case Icon::Trash:
        painter.drawLine(QPointF(4, 7), QPointF(20, 7));
        painter.drawPolyline(
            QPolygonF({QPointF(9, 7), QPointF(9, 4.5), QPointF(15, 4.5), QPointF(15, 7)}));
        painter.drawPolyline(QPolygonF({QPointF(6, 7), QPointF(7, 20), QPointF(17, 20),
                                        QPointF(18, 7)}));
        painter.drawLine(QPointF(10, 11), QPointF(10, 16.5));
        painter.drawLine(QPointF(14, 11), QPointF(14, 16.5));
        break;

    case Icon::Refresh:
        painter.drawArc(QRectF(4, 4, 16, 16), 40 * 16, 280 * 16);
        painter.drawPolyline(
            QPolygonF({QPointF(16.5, 2.8), QPointF(20, 5.2), QPointF(16.2, 7.6)}));
        break;

    case Icon::Play:
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawPolygon(QPolygonF({QPointF(8, 5), QPointF(19, 12), QPointF(8, 19)}));
        break;

    case Icon::Copy:
        painter.drawRoundedRect(QRectF(8.5, 8.5, 12, 12), 2.5, 2.5);
        painter.drawPolyline(QPolygonF({QPointF(5.5, 15.5), QPointF(3.5, 15.5),
                                        QPointF(3.5, 3.5), QPointF(15.5, 3.5),
                                        QPointF(15.5, 5.5)}));
        break;

    case Icon::Info:
        painter.drawEllipse(QPointF(12, 12), 8.8, 8.8);
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawEllipse(QPointF(12, 8), 1.3, 1.3);
        painter.setPen(QPen(color, 1.9, Qt::SolidLine, Qt::RoundCap));
        painter.drawLine(QPointF(12, 11), QPointF(12, 16.5));
        break;

    case Icon::Warning: {
        QPainterPath path;
        path.moveTo(12, 3.5);
        path.lineTo(21.5, 20);
        path.lineTo(2.5, 20);
        path.closeSubpath();
        painter.drawPath(path);
        painter.setPen(QPen(color, 1.9, Qt::SolidLine, Qt::RoundCap));
        painter.drawLine(QPointF(12, 9), QPointF(12, 14));
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawEllipse(QPointF(12, 17), 1.2, 1.2);
        break;
    }

    case Icon::Stop:
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawRoundedRect(QRectF(6, 6, 12, 12), 2.5, 2.5);
        break;

    case Icon::Sun:
        painter.drawEllipse(QPointF(12, 12), 4.2, 4.2);
        for (int i = 0; i < 8; ++i) {
            const double angle = i * kPi / 4.0;
            const QPointF a(12 + std::cos(angle) * 7.2, 12 + std::sin(angle) * 7.2);
            const QPointF b(12 + std::cos(angle) * 9.4, 12 + std::sin(angle) * 9.4);
            painter.drawLine(a, b);
        }
        break;

    case Icon::Moon: {
        QPainterPath path;
        path.addEllipse(QRectF(4, 3.5, 16, 17));
        QPainterPath cut;
        cut.addEllipse(QRectF(9, 1, 14, 15));
        painter.drawPath(path.subtracted(cut));
        break;
    }

    case Icon::About:
        painter.drawEllipse(QPointF(12, 12), 8.8, 8.8);
        painter.setBrush(color);
        painter.setPen(Qt::NoPen);
        painter.drawEllipse(QPointF(12, 8), 1.3, 1.3);
        painter.setPen(QPen(color, 1.9, Qt::SolidLine, Qt::RoundCap));
        painter.drawLine(QPointF(12, 11), QPointF(12, 16.5));
        break;

    case Icon::Check:
        painter.drawPolyline(QPolygonF({QPointF(5, 12.5), QPointF(10, 17.5), QPointF(19, 6.5)}));
        break;

    case Icon::Clear:
        painter.drawLine(QPointF(4, 20), QPointF(16, 20));
        painter.drawPolyline(QPolygonF({QPointF(7, 16), QPointF(16.5, 6.5),
                                        QPointF(20.5, 10.5), QPointF(11, 20)}));
        break;

    case Icon::Globe:
        painter.drawEllipse(QPointF(12, 12), 8.8, 8.8);
        painter.drawEllipse(QPointF(12, 12), 4, 8.8);
        painter.drawLine(QPointF(3.2, 12), QPointF(20.8, 12));
        break;

    case Icon::ChevronDown:
        painter.drawPolyline(
            QPolygonF({QPointF(6.5, 9.5), QPointF(12, 15), QPointF(17.5, 9.5)}));
        break;
    }
}

} // namespace

namespace Icons {

QPixmap pixmap(Icon id, const QColor &color, int size)
{
    QPixmap pixmap(size, size);
    pixmap.fill(Qt::transparent);
    QPainter painter(&pixmap);
    painter.scale(size / 24.0, size / 24.0);
    drawIcon(painter, id, color);
    return pixmap;
}

QIcon icon(Icon id, const QColor &color, int size)
{
    QIcon result;
    for (int scale = 1; scale <= 2; ++scale)
        result.addPixmap(pixmap(id, color, size * scale));
    return result;
}

} // namespace Icons
