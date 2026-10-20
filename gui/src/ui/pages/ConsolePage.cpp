#include "ui/pages/ConsolePage.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QStackedWidget>
#include <QTextEdit>
#include <QVBoxLayout>

#include "ui/I18n.h"
#include "ui/Icons.h"
#include "ui/Theme.h"

ConsolePage::ConsolePage(PageHost *pageHost, QWidget *parent)
    : Page(pageHost, parent)
{
    addPageHeader(QStringLiteral("控制台"),
                  QStringLiteral("USB 收发日志与最近一次操作的完整 JSON"));

    auto *logCard = Ui::card(this);
    QVBoxLayout *logBody = Ui::cardBody(logCard);
    auto *toolbar = new QHBoxLayout;
    toolbar->setSpacing(10);

    m_verbose = new ToggleSwitch(state().verbose, logCard);
    toolbar->addWidget(m_verbose);
    toolbar->addWidget(Ui::label(QStringLiteral("详细 USB 日志"), {}, logCard));
    toolbar->addSpacing(16);
    m_autoScroll = new ToggleSwitch(state().autoScrollLogs, logCard);
    toolbar->addWidget(m_autoScroll);
    toolbar->addWidget(Ui::label(QStringLiteral("自动滚动"), {}, logCard));
    toolbar->addStretch(1);

    m_count = Ui::chip(QStringLiteral("0"), Tone::Neutral, logCard);
    toolbar->addWidget(m_count);

    auto *clear = Ui::button(QStringLiteral("清空日志"), Ui::ButtonVariant::Text, Icon::Trash,
                             logCard);
    toolbar->addWidget(clear);
    logBody->addLayout(toolbar);

    m_logView = new QTextEdit(logCard);
    m_logView->setObjectName(QStringLiteral("logView"));
    m_logView->setReadOnly(true);
    m_logView->setAcceptRichText(true);
    m_logView->setLineWrapMode(QTextEdit::NoWrap);
    m_logView->setMinimumHeight(280);
    m_logView->setPlaceholderText(
        I18n::t(QStringLiteral("暂无日志。启用“详细 USB 日志”可看到 TX / RX 帧。")));
    m_logView->setProperty("i18nPlaceholder",
                           QStringLiteral("暂无日志。启用“详细 USB 日志”可看到 TX / RX 帧。"));
    logBody->addWidget(m_logView);
    m_content->addWidget(logCard);

    auto *jsonCard = Ui::card(this);
    QVBoxLayout *jsonBody = Ui::cardBody(jsonCard);
    auto *jsonHeader = new QHBoxLayout;
    jsonHeader->setSpacing(8);
    jsonHeader->addWidget(Ui::cardHeader(Icon::Terminal, QStringLiteral("完整 JSON"),
                                         QStringLiteral("最近一次操作"), jsonCard),
                          1);
    auto *copy = Ui::button(QStringLiteral("复制"), Ui::ButtonVariant::Text, Icon::Copy, jsonCard);
    jsonHeader->addWidget(copy, 0, Qt::AlignTop);
    jsonBody->addLayout(jsonHeader);

    m_jsonStack = new QStackedWidget(jsonCard);
    m_jsonStack->addWidget(Ui::emptyState(
        Icon::Terminal, QStringLiteral("没有 JSON 结果"),
        QStringLiteral("执行一次操作后，完整协议结果会显示在这里"), m_jsonStack));
    m_jsonView = Ui::codeBlock(QString(), 360, m_jsonStack);
    m_jsonStack->addWidget(m_jsonView);
    jsonBody->addWidget(m_jsonStack);
    m_content->addWidget(jsonCard);
    m_content->addStretch(1);

    connect(m_verbose, &ToggleSwitch::toggled, this,
            [this](bool checked) { state().verbose = checked; });
    connect(m_autoScroll, &ToggleSwitch::toggled, this,
            [this](bool checked) { state().autoScrollLogs = checked; });
    connect(clear, &QPushButton::clicked, this, [this] {
        state().logs.clear();
        state().appendLog(I18n::t(QStringLiteral("日志已清空")));
        rebuildLog();
        updateCounter();
    });
    connect(copy, &QPushButton::clicked, this, [this] {
        if (!state().hasResult)
            return;
        QGuiApplication::clipboard()->setText(state().prettyJson);
        host()->showToast(I18n::t(QStringLiteral("JSON 已复制到剪贴板")), Tone::Success);
    });
}

QString ConsolePage::lineHtml(const LogLine &line) const
{
    const Palette &palette = Theme::instance().palette();
    QColor color = palette.onSurface;
    if (line.text.startsWith(QLatin1String("TX->")))
        color = palette.primary;
    else if (line.text.startsWith(QLatin1String("RX->")))
        color = palette.tertiary;
    else if (line.text.contains(QLatin1String("[WARN]")))
        color = palette.error;
    else if (line.text.contains(QLatin1String("[INFO]")))
        color = palette.onSurfaceVariant;

    const QString text = QStringLiteral("%1s  %2")
                             .arg(line.at, 8, 'f', 2, QLatin1Char(' '))
                             .arg(line.text.toHtmlEscaped());
    return QStringLiteral("<span style=\"color:%1; white-space:pre;\">%2</span>")
        .arg(color.name(), text);
}

void ConsolePage::rebuildLog()
{
    QString html;
    html.reserve(state().logs.size() * 96);
    for (const LogLine &line : state().logs)
        html += lineHtml(line) + QStringLiteral("<br>");
    m_logView->setHtml(html);
    if (state().autoScrollLogs)
        m_logView->verticalScrollBar()->setValue(m_logView->verticalScrollBar()->maximum());
}

void ConsolePage::appendLine(const LogLine &line)
{
    m_logView->append(lineHtml(line));
    if (state().autoScrollLogs)
        m_logView->verticalScrollBar()->setValue(m_logView->verticalScrollBar()->maximum());
    updateCounter();
}

void ConsolePage::rebuildJson()
{
    if (state().hasResult && !state().prettyJson.isEmpty()) {
        m_jsonView->setPlainText(state().prettyJson);
        m_jsonStack->setCurrentIndex(1);
    } else {
        m_jsonStack->setCurrentIndex(0);
    }
}

void ConsolePage::updateCounter()
{
    m_count->setText(I18n::tf(QStringLiteral("{count} 条"),
                              {{QStringLiteral("count"), state().logs.size()}}));
}

void ConsolePage::refresh()
{
    {
        const QSignalBlocker b1(m_verbose);
        const QSignalBlocker b2(m_autoScroll);
        m_verbose->setChecked(state().verbose, false);
        m_autoScroll->setChecked(state().autoScrollLogs, false);
    }
    rebuildLog();
    rebuildJson();
    updateCounter();
}
