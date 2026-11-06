// Ordinary Qt file-dialog caller. Portal routing belongs to Qt's platform theme.
#include <QApplication>
#include <QFile>
#include <QFileDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QTimer>
#include <QVBoxLayout>
#include <QWidget>
#include <cstdio>

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    if (argc != 3) return 2;
    const QString mode = QString::fromLocal8Bit(argv[1]);
    if (mode != "open" && mode != "save" && mode != "cancel") return 2;
    app.setApplicationName(QStringLiteral("Canopa Qt file chooser probe"));
    app.setDesktopFileName(QStringLiteral("org.canopa.FileChooserQt"));
    QWidget parent;
    parent.setWindowTitle(QStringLiteral("Qt file chooser parent"));
    parent.resize(420, 180);
    auto *layout = new QVBoxLayout(&parent);
    layout->addWidget(new QLabel(QStringLiteral("This application calls QFileDialog.")));
    parent.show();
    QFileDialog *active = nullptr;
    bool timedOut = false;
    QTimer::singleShot(45000, &app, [&] { timedOut = true; if (active) active->reject(); app.exit(2); });
    QTimer::singleShot(150, &app, [&] {
        // Model a user opening a dialog after application startup. Qt's portal
        // theme discovers FileChooser support asynchronously during startup.
        QFileDialog dialog(&parent, QStringLiteral("Ordinary Qt file chooser"), QString::fromLocal8Bit(argv[2]));
        active = &dialog;
        dialog.setWindowModality(Qt::WindowModal);
        dialog.setNameFilters({QStringLiteral("Text documents (*.txt)"), QStringLiteral("All files (*)")});
        dialog.selectNameFilter(QStringLiteral("Text documents (*.txt)"));
        dialog.setAcceptMode(mode == "save" ? QFileDialog::AcceptSave : QFileDialog::AcceptOpen);
        dialog.setFileMode(mode == "save" ? QFileDialog::AnyFile : QFileDialog::ExistingFile);
        if (mode == "save") dialog.selectFile(QString::fromUtf8("Qt saved été.txt"));
        const int result = dialog.exec();
        active = nullptr;
        if (timedOut) { app.exit(2); return; }
        QJsonArray paths;
        QString contents;
        if (result == QDialog::Accepted) {
            const QStringList selected = dialog.selectedFiles();
            if (selected.size() != 1) { app.exit(3); return; }
            for (const auto &path : selected) paths.append(path);
            QFile file(selected.constFirst());
            if (mode == "save") {
                const QByteArray bytes("Written by an ordinary Qt file dialog caller\n");
                if (!file.open(QIODevice::WriteOnly | QIODevice::NewOnly) || file.write(bytes) != bytes.size() || !file.flush()) {
                    app.exit(4); return;
                }
                contents = QString::fromUtf8(bytes);
            } else {
                if (!file.open(QIODevice::ReadOnly)) { app.exit(5); return; }
                contents = QString::fromUtf8(file.readAll());
            }
        }
        const QJsonObject evidence{{"qt_version", QString::fromLatin1(qVersion())}, {"mode", mode},
            {"dialog_constructed_after_event_loop", true},
            {"dialog_result", result}, {"paths", paths}, {"contents", contents},
            {"selected_filter", dialog.selectedNameFilter()}};
        std::puts(QJsonDocument(evidence).toJson(QJsonDocument::Compact).constData());
        app.exit(0);
    });
    return app.exec();
}
