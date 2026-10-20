import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// The window's header: menus, search, total rates and connection status.
Rectangle {
    id: toolbar

    signal importRequested()
    signal aboutRequested()

    // No answer from the daemon yet, as opposed to a failed one.
    readonly property bool connecting: !WyreModel.connected && WyreModel.error === ""

    height: Theme.gridUnit * 2.8
    color: Theme.raisedBackground

    Kirigami.Separator {
        anchors.bottom: parent.bottom
        width: parent.width
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.largeSpacing
        anchors.rightMargin: Theme.largeSpacing
        spacing: Theme.largeSpacing

        Row {
            QQC2.ToolButton {
                text: "File"
                onClicked: fileMenu.popup()

                QQC2.Menu {
                    id: fileMenu

                    QQC2.Menu {
                        title: "Geolocation"

                        GeoModeItem {
                            text: "No Geolocation"
                            mode: "Off"
                        }
                        GeoModeItem {
                            text: "Free Databases"
                            mode: "Free"
                        }
                        GeoModeItem {
                            text: "DB-IP Databases"
                            mode: "DbIp"
                        }
                        // With none imported yet, go straight to importing.
                        GeoModeItem {
                            text: "Imported Databases"
                            mode: "Imported"
                            onTriggered: {
                                if (JSON.parse(WyreModel.imported_json).length === 0)
                                    toolbar.importRequested()
                            }
                        }
                        QQC2.MenuSeparator {}
                        QQC2.MenuItem {
                            text: "Status: " + WyreModel.geo_status
                            enabled: false
                        }
                        QQC2.MenuItem {
                            text: "Check for updates now"
                            enabled: WyreModel.geo_mode !== "Off"
                            onTriggered: WyreModel.refresh_geo()
                        }
                        QQC2.MenuSeparator {}
                        QQC2.MenuItem {
                            text: "Import Databases…"
                            onTriggered: toolbar.importRequested()
                        }
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        text: "About"
                        onTriggered: toolbar.aboutRequested()
                    }
                }
            }

            QQC2.ToolButton {
                text: "View"
                onClicked: viewMenu.popup()

                QQC2.Menu {
                    id: viewMenu

                    QQC2.Menu {
                        title: "Protocols"

                        QQC2.MenuItem {
                            text: "TCP"
                            checkable: true
                            checked: WyreModel.show_tcp
                            onTriggered: Rows.changeView(() => WyreModel.toggle_protocol("tcp"))
                        }
                        QQC2.MenuItem {
                            text: "UDP"
                            checkable: true
                            checked: WyreModel.show_udp
                            onTriggered: Rows.changeView(() => WyreModel.toggle_protocol("udp"))
                        }
                        QQC2.MenuItem {
                            text: "ICMP"
                            checkable: true
                            checked: WyreModel.show_icmp
                            onTriggered: Rows.changeView(() => WyreModel.toggle_protocol("icmp"))
                        }
                    }
                    // Ports 53 and 5353 (DNS and mDNS).
                    QQC2.MenuItem {
                        text: "Hide DNS Connections"
                        checkable: true
                        checked: WyreModel.hide_dns
                        onTriggered: Rows.changeView(() => WyreModel.toggle_dns())
                    }
                    QQC2.MenuItem {
                        text: "Hide Connections Without Traffic"
                        checkable: true
                        checked: WyreModel.hide_empty
                        onTriggered: Rows.changeView(() => WyreModel.toggle_empty())
                    }
                    QQC2.MenuSeparator {}
                    QQC2.MenuItem {
                        action: zoomInAction
                    }
                    QQC2.MenuItem {
                        action: zoomOutAction
                    }
                    QQC2.MenuItem {
                        action: resetZoomAction
                    }
                }
            }
        }

        ToolbarSeparator {}

        // Matches program names and connection addresses.
        Kirigami.SearchField {
            id: searchField
            Layout.fillWidth: true
            Layout.minimumWidth: Theme.gridUnit * 8
            onTextChanged: searchDelay.restart()
        }

        // Debounced, so fast typing doesn't lag.
        Timer {
            id: searchDelay
            interval: 150
            onTriggered: Rows.changeView(() => WyreModel.set_search(searchField.text))
        }

        Total {
            label: "DOWNLOAD"
            value: WyreModel.total_recv_str + "/s"
            color: Theme.download
        }

        Total {
            label: "UPLOAD"
            value: WyreModel.total_send_str + "/s"
            color: Theme.upload
        }

        ToolbarSeparator {
            visible: !WyreModel.connected
        }

        Row {
            visible: !WyreModel.connected
            spacing: Theme.smallSpacing

            Rectangle {
                id: statusDot
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.smallSpacing * 1.75
                height: width
                radius: width / 2
                color: toolbar.connecting ? Theme.dimText : Theme.negative
                Behavior on color { ColorAnimation { duration: Kirigami.Units.longDuration } }

                SequentialAnimation on opacity {
                    running: !WyreModel.connected
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.2; duration: 600 }
                    NumberAnimation { to: 1.0; duration: 600 }
                    // Stopping mid-fade would leave the dot half transparent.
                    onRunningChanged: if (!running) statusDot.opacity = 1
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: toolbar.connecting ? "connecting…" : "offline"
                font.family: Theme.monoFamily
                font.pointSize: Theme.normalSize
                color: toolbar.connecting ? Theme.dimText : Theme.negative
                Behavior on color { ColorAnimation { duration: Kirigami.Units.longDuration } }
            }
        }
    }

    // The View menu's zoom items. Out here, so their shortcuts work while the
    // menu is closed.
    QQC2.Action {
        id: zoomInAction
        text: "Zoom In"
        shortcut: "Ctrl++"
        onTriggered: WyreModel.zoom_in()
    }
    QQC2.Action {
        id: zoomOutAction
        text: "Zoom Out"
        shortcut: "Ctrl+-"
        onTriggered: WyreModel.zoom_out()
    }
    QQC2.Action {
        id: resetZoomAction
        text: "Reset Zoom"
        shortcut: "Ctrl+0"
        enabled: Theme.zoom !== 1
        onTriggered: WyreModel.reset_zoom()
    }
    // Ctrl++ takes Shift on many keyboards, so Ctrl+= zooms in too, as in
    // browsers.
    Shortcut {
        sequence: "Ctrl+="
        onActivated: zoomInAction.trigger()
    }

    component ToolbarSeparator: Kirigami.Separator {
        Layout.fillHeight: true
        Layout.topMargin: Theme.smallSpacing
        Layout.bottomMargin: Theme.smallSpacing
    }

    component GeoModeItem: QQC2.MenuItem {
        required property string mode

        checkable: true
        autoExclusive: true
        checked: WyreModel.geo_mode === mode
        onTriggered: WyreModel.select_geo_mode(mode)
    }

    // A total rate, at a fixed width so the toolbar doesn't shift as it
    // changes.
    component Total: RowLayout {
        id: total

        property string label
        property string value
        property color color

        spacing: Theme.largeSpacing

        ToolbarSeparator {}

        Column {
            Layout.preferredWidth: Theme.gridUnit * 6
            Layout.minimumWidth: Theme.gridUnit * 6
            Layout.maximumWidth: Theme.gridUnit * 6
            spacing: Theme.smallSpacing / 2

            Text {
                text: total.label
                font.family: Theme.monoFamily
                font.pointSize: Theme.tinySize
                font.letterSpacing: 1.8 * Theme.zoom
                font.bold: true
                color: Theme.dimText
            }
            Text {
                text: total.value
                font.family: Theme.monoFamily
                font.pointSize: Theme.mediumSize
                font.bold: true
                color: total.color
            }
        }
    }
}
