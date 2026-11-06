// Application-side Qt DBus/PDF probe. This explicitly calls the Print portal;
// it does not claim that Qt's platform QPrintDialog uses that portal.
#include <QApplication>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusUnixFileDescriptor>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPageLayout>
#include <QPainter>
#include <QPrinter>
#include <QTimer>
#include <cstdio>

class Probe final : public QObject {
    Q_OBJECT
    QDBusConnection bus = QDBusConnection::sessionBus();
    const QString service = QStringLiteral("org.freedesktop.portal.Desktop");
    const QString path = QStringLiteral("/org/freedesktop/portal/desktop");
    const QString interface = QStringLiteral("org.freedesktop.portal.Print");
    QString prepareHandle, printHandle;
    QFile document;
    QJsonObject evidence;

    void fail(const QString &message) {
        std::fprintf(stderr, "%s\n", qPrintable(message));
        QCoreApplication::exit(1);
    }
    QString call(const QString &method, const QList<QVariant> &arguments) {
        auto message = QDBusMessage::createMethodCall(service, path, interface, method);
        message.setArguments(arguments);
        auto reply = bus.call(message, QDBus::Block, 5000);
        if (reply.type() == QDBusMessage::ErrorMessage || reply.arguments().size() != 1) {
            fail(reply.errorName() + QStringLiteral(": ") + reply.errorMessage());
            return {};
        }
        return qvariant_cast<QDBusObjectPath>(reply.arguments().first()).path();
    }
public:
    Probe() {
        document.setFileName(qEnvironmentVariable("CANOPA_PRINT_QT_DOCUMENT"));
        if (!bus.connect(service, QString(), QStringLiteral("org.freedesktop.portal.Request"),
                         QStringLiteral("Response"), this,
                         SLOT(response(uint,QVariantMap,QDBusMessage))))
            qFatal("Cannot subscribe to portal responses");
        QTimer::singleShot(0, this, &Probe::begin);
        QTimer::singleShot(60000, this, [this] { fail(QStringLiteral("Print probe timed out")); });
    }
private slots:
    void begin() {
        QVariantMap settings{{QStringLiteral("n-copies"), QStringLiteral("2")},
                             {QStringLiteral("collate"), qEnvironmentVariable("CANOPA_PRINT_QT_COLLATE", "true")},
                             {QStringLiteral("number-up"), QStringLiteral("1")}};
        QVariantMap paper{{QStringLiteral("Name"), QStringLiteral("iso_a4")},
                          {QStringLiteral("DisplayName"), QStringLiteral("A4")},
                          {QStringLiteral("Width"), 210.}, {QStringLiteral("Height"), 297.}};
        prepareHandle = call(QStringLiteral("PreparePrint"),
            {qEnvironmentVariable("CANOPA_PRINT_QT_PARENT"), QStringLiteral("Qt print probe"),
             settings, paper, QVariantMap{{QStringLiteral("handle_token"), QStringLiteral("qtprepare")}}});
    }
    void response(uint code, const QVariantMap &values, const QDBusMessage &message) {
        if (message.path() != prepareHandle && message.path() != printHandle)
            return;
        if (code != 0) {
            fail(QStringLiteral("Portal response %1").arg(code));
            return;
        }
        if (message.path() == printHandle) {
            evidence.insert(QStringLiteral("print_response"), int(code));
            auto json = QJsonDocument(evidence).toJson(QJsonDocument::Compact);
            std::puts(json.constData());
            QCoreApplication::exit(0);
            return;
        }
        const auto settings = qdbus_cast<QVariantMap>(values.value(QStringLiteral("settings")));
        const auto paper = qdbus_cast<QVariantMap>(values.value(QStringLiteral("page-setup")));
        const uint token = values.value(QStringLiteral("token")).toUInt();
        if (settings.value(QStringLiteral("n-copies")).toString() != QStringLiteral("1") || token == 0) {
            fail(QStringLiteral("Printer approval did not request one render copy"));
            return;
        }
        evidence.insert(QStringLiteral("settings"), QJsonObject::fromVariantMap(settings));
        evidence.insert(QStringLiteral("page_setup"), QJsonObject::fromVariantMap(paper));
        evidence.insert(QStringLiteral("qt_version"), QString::fromLatin1(qVersion()));
        {
            QPrinter printer;
            printer.setOutputFormat(QPrinter::PdfFormat);
            printer.setOutputFileName(document.fileName());
            printer.setResolution(72);
            printer.setCopyCount(settings.value(QStringLiteral("n-copies")).toInt());
            printer.setCollateCopies(settings.value(QStringLiteral("collate")).toString() == QStringLiteral("true"));
            const QPageSize size(QSizeF(paper.value(QStringLiteral("Width")).toDouble(),
                                       paper.value(QStringLiteral("Height")).toDouble()), QPageSize::Millimeter);
            if (!size.isValid() || !printer.setPageLayout(QPageLayout(size, QPageLayout::Portrait,
                                                                     QMarginsF(), QPageLayout::Millimeter))) {
                fail(QStringLiteral("Invalid approved Qt page geometry"));
                return;
            }
            printer.setFullPage(true);
            QPainter painter;
            if (!painter.begin(&printer)) {
                fail(QStringLiteral("Cannot start Qt PDF painter"));
                return;
            }
            QFont font(QStringLiteral("sans"));
            font.setPixelSize(24);
            painter.setFont(font);
            for (int page = 1; page <= 3; ++page) {
                if (page > 1 && !printer.newPage()) {
                    fail(QStringLiteral("Cannot append Qt PDF page"));
                    return;
                }
                painter.drawText(40, 60, QStringLiteral("PAGE-%1").arg(page));
            }
            painter.end();
        }
        if (!document.open(QIODevice::ReadOnly)) {
            fail(document.errorString());
            return;
        }
        printHandle = call(QStringLiteral("Print"),
            {qEnvironmentVariable("CANOPA_PRINT_QT_PARENT"), QStringLiteral("Qt prepared delivery"),
             QVariant::fromValue(QDBusUnixFileDescriptor(document.handle())),
             QVariantMap{{QStringLiteral("token"), token},
                         {QStringLiteral("handle_token"), QStringLiteral("qtprint")}}});
    }
};

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    Probe probe;
    return app.exec();
}
#include "print-qt-render.moc"
