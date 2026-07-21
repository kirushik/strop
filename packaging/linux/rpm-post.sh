#!/bin/sh
command -v update-mime-database >/dev/null 2>&1 && \
    update-mime-database /usr/share/mime || :
command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database /usr/share/applications || :
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
    gtk-update-icon-cache -q -t /usr/share/icons/hicolor || :
