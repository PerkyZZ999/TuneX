import QtQuick

// Application shell placeholder (S1 W-001b).
// The real window, navigation, and theme land in W-003 from docs/DESIGN.md;
// this bare window exists only so the QML module has a compilable root.
Window {
    id: root

    minimumWidth: 960
    minimumHeight: 640
    width: 960
    height: 640
    visible: true
    title: qsTr("TuneX")
}
