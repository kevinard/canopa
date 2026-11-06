// Application-side QPrintDialog probe. It uses public Qt APIs and makes no
// explicit portal calls; the native fixture observes the toolkit's own routing.
#include <QApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QPageSize>
#include <QPainter>
#include <QPrintDialog>
#include <QPrinter>
#include <QTimer>
#include <QVBoxLayout>
#include <QWidget>
#include <cstdio>

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("Canopa Qt native print probe"));
    QWidget parent;
    parent.setWindowTitle(QStringLiteral("Qt automatic print parent"));
    parent.resize(420, 180);
    auto *layout = new QVBoxLayout(&parent);
    layout->addWidget(new QLabel(QStringLiteral("This application calls QPrintDialog.")));
    parent.show();

    QPrinter printer(QPrinter::ScreenResolution);
    printer.setPrinterName(qEnvironmentVariable("CANOPA_PRINT_QT_QUEUE"));
    printer.setDocName(QStringLiteral("Qt automatic print probe"));
    printer.setPageSize(QPageSize(QPageSize::A4));
    printer.setCopyCount(1);
    QPrintDialog dialog(&printer, &parent);
    dialog.setWindowTitle(QStringLiteral("Qt automatic print dialog"));
    bool timedOut = false;
    QTimer::singleShot(60000, &app, [&] {
        timedOut = true;
        std::fprintf(stderr, "Qt native print probe timed out\n");
        dialog.reject();
        app.exit(2);
    });
    QTimer::singleShot(100, &app, [&] {
        const int result = dialog.exec();
        if (timedOut) {
            app.exit(2);
            return;
        }
        QJsonObject evidence{
            {QStringLiteral("qt_version"), QString::fromLatin1(qVersion())},
            {QStringLiteral("dialog_result"), result},
            {QStringLiteral("printer"), printer.printerName()},
            {QStringLiteral("output_format"), int(printer.outputFormat())},
            {QStringLiteral("copies"), printer.copyCount()},
            {QStringLiteral("pages_drawn"), 0},
        };
        if (result == QDialog::Accepted) {
            QPainter painter;
            if (!painter.begin(&printer)) {
                std::fprintf(stderr, "Cannot begin native Qt printing\n");
                app.exit(1);
                return;
            }
            QFont font(QStringLiteral("sans"));
            font.setPixelSize(24);
            painter.setFont(font);
            for (int page = 1; page <= 3; ++page) {
                if (page > 1 && !printer.newPage()) {
                    std::fprintf(stderr, "Cannot append native Qt page\n");
                    app.exit(1);
                    return;
                }
                painter.drawText(40, 60, QStringLiteral("QT-NATIVE-PAGE-%1").arg(page));
            }
            if (!painter.end()) {
                std::fprintf(stderr, "Cannot finish native Qt printing\n");
                app.exit(1);
                return;
            }
            evidence.insert(QStringLiteral("pages_drawn"), 3);
        }
        const auto json = QJsonDocument(evidence).toJson(QJsonDocument::Compact);
        std::puts(json.constData());
        app.exit(0);
    });
    return app.exec();
}
