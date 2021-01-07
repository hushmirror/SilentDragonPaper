// Copyright (c) 2016-2020 The Hush developers
// Distributed under the GPLv3 software license, see the accompanying
// file COPYING or https://www.gnu.org/licenses/gpl-3.0.en.html
#include "mainwindow.h"
#include "version.h"
#include <QApplication>

int main(int argc, char *argv[])
{
    QCoreApplication::setAttribute(Qt::AA_UseHighDpiPixmaps);
    QCoreApplication::setAttribute(Qt::AA_EnableHighDpiScaling);

    QCoreApplication::setOrganizationDomain("hush.is");
    QCoreApplication::setOrganizationName("Hush");

    #ifdef Q_OS_LINUX
        QFontDatabase::addApplicationFont(":/fonts/res/Ubuntu-R.ttf");
        qApp->setFont(QFont("Ubuntu", 11, QFont::Normal, false));
    #endif

    QApplication a(argc, argv);
    MainWindow w;

    w.setWindowTitle(QString("Extreme Privacy: SilentDragonPaper ") + APP_VERSION);

    w.show();

    return a.exec();
}
