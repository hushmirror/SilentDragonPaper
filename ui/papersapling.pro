#-------------------------------------------------
#
# Project created by QtCreator 2019-05-23T09:37:52
#
#-------------------------------------------------

QT       += core gui

greaterThan(QT_MAJOR_VERSION, 4): QT += widgets

TARGET = hushpaperwalletui
TEMPLATE = app

MOC_DIR = bin
OBJECTS_DIR = bin
UI_DIR = src
DEFINES += QT_DEPRECATED_WARNINGS

CONFIG += c++14
CONFIG += precompile_header

PRECOMPILED_HEADER = src/precompiled.h


SOURCES += \
        src/main.cpp \
        src/mainwindow.cpp \
        src/qrcodelabel.cpp \
        src/qrcode/BitBuffer.cpp \
        src/qrcode/QrCode.cpp \
        src/qrcode/QrSegment.cpp 

HEADERS += \
        src/mainwindow.h \
        src/qrcodelabel.h \
        src/precompiled.h \
        src/qrcode/BitBuffer.hpp \
        src/qrcode/QrCode.hpp \
        src/qrcode/QrSegment.hpp \
        src/version.h \
        qtlib/src/hushpaperrust.h 


FORMS += \
        src/about.ui \
        src/mainwindow.ui \
        src/wallet.ui

# Rust library
INCLUDEPATH += $$PWD/qtlib/src
DEPENDPATH  += $$PWD/qtlib/src

mac: LIBS+= -Wl,-dead_strip
mac: LIBS+= -Wl,-dead_strip_dylibs
mac: LIBS+= -Wl,-bind_at_load

win32: RC_ICONS = res/icon.ico
ICON = res/logo.icns

unix:        librust.target   = $$PWD/qtlib/target/release/libhushpaperrust.a
else:win32:  librust.target   = $$PWD/qtlib/target/x86_64-pc-windows-gnu/release/hushpaperrust.lib

unix:        librust.commands = $(MAKE) -C $$PWD/qtlib 
else:win32:  librust.commands = $(MAKE) -C $$PWD/qtlib winrelease

librustclean.commands = "rm -rf $$PWD/qtlib/target"
distclean.depends += librustclean

QMAKE_INFO_PLIST = res/Info.plist

QMAKE_EXTRA_TARGETS += librust librustclean distclean
QMAKE_CLEAN += $$PWD/qtlib/target/release/libhushpaperrust.a

# Default rules for deployment.
qnx: target.path = /tmp/$${TARGET}/bin
else: unix:!android: target.path = /opt/$${TARGET}/bin
!isEmpty(target.path): INSTALLS += target


win32: LIBS += -L$$PWD/qtlib/target/x86_64-pc-windows-gnu/release -lhushpaperrust
else:macx: LIBS += -L$$PWD/qtlib/target/release -lhushpaperrust -framework Security -framework Foundation
else:unix: LIBS += -L$$PWD/qtlib/target/release -lhushpaperrust -ldl

win32: PRE_TARGETDEPS += $$PWD/qtlib/target/x86_64-pc-windows-gnu/release/hushpaperrust.lib
else:unix::PRE_TARGETDEPS += $$PWD/qtlib/target/release/libhushpaperrust.a
