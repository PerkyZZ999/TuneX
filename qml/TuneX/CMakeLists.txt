# TuneX QML module (URI TuneX). Backing target named in snake_case to avoid
# executable/module name clashes on case-insensitive filesystems.
qt_add_library(tunex_qml STATIC)

qt_add_qml_module(tunex_qml
    URI TuneX
    QML_FILES
        App.qml
)

target_link_libraries(tunex_qml PRIVATE Qt6::Quick)
