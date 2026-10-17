import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// The selected connection in full, in a pane that slides open beside the
// list. A closed connection stays, marked closed, until another is picked.
Item {
    id: pane

    readonly property var connection: Rows.selected ?? blank
    readonly property bool gone: connection.gone === true
    readonly property string location: connection.location ?? ""
    readonly property string country: connection.country ?? ""

    // Something for the hidden contents' bindings to read while nothing is
    // selected.
    readonly property var blank: ({
        proto: "", remote: "", local: "", proc_name: "", pid: 0, ppid: 0, state: null,
        recv: "", send: "", recv_b: 0, send_b: 0, recv_total: "", send_total: ""
    })

    implicitWidth: Rows.selected !== null ? Theme.paneWidth : 0
    clip: true
    Behavior on implicitWidth { NumberAnimation { duration: Kirigami.Units.longDuration; easing.type: Easing.OutCubic } }

    Rectangle {
        width: Theme.paneWidth
        height: parent.height
        color: Theme.alternateBackground

        ColumnLayout {
            anchors.fill: parent
            spacing: 0
            visible: Rows.selected !== null

            // Header: the connection and its process.
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Theme.gridUnit * 3
                color: Theme.raisedBackground

                Kirigami.Separator {
                    anchors.bottom: parent.bottom
                    width: parent.width
                }
                // Grey, like a gone row's stripe.
                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 2
                    color: Theme.strongLine
                    visible: pane.gone
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.largeSpacing
                    anchors.rightMargin: Theme.largeSpacing
                    anchors.topMargin: Theme.smallSpacing * 2
                    anchors.bottomMargin: Theme.smallSpacing * 2
                    spacing: Theme.smallSpacing

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.largeSpacing

                        ProtocolBadge {
                            Layout.preferredHeight: Theme.gridUnit * 0.9
                            padding: Theme.largeSpacing
                            pointSize: Theme.smallSize
                            protocol: pane.connection.proto
                        }
                        // Clipped; the field below shows it whole.
                        SelectableText {
                            Layout.fillWidth: true
                            clip: true
                            text: pane.connection.remote
                            font.pointSize: Theme.normalSize
                            font.bold: true
                            color: pane.gone ? Theme.dimText : Theme.address
                        }
                        Rectangle {
                            visible: pane.gone
                            Layout.preferredWidth: closedLabel.implicitWidth + Theme.smallSpacing * 2.5
                            Layout.preferredHeight: Theme.gridUnit * 0.85
                            radius: Theme.cornerRadius
                            color: Theme.closedBadge

                            Text {
                                id: closedLabel
                                anchors.centerIn: parent
                                text: "CLOSED"
                                font.family: Theme.monoFamily
                                font.pointSize: Theme.tinySize
                                font.bold: true
                                color: Theme.dimText
                            }
                        }
                        QQC2.ToolButton {
                            implicitWidth: Theme.gridUnit * 1.4
                            implicitHeight: implicitWidth
                            icon.width: Theme.smallIconSize
                            icon.height: Theme.smallIconSize
                            icon.name: "window-close-symbolic"
                            onClicked: Rows.selected = null
                        }
                    }

                    // The name is elided to keep the ids in view. A ppid of 0
                    // is unknown.
                    SelectableText {
                        id: processLine
                        Layout.fillWidth: true
                        clip: true
                        text: processName.elidedText + processIds.text
                        font.pointSize: Theme.normalSize
                        color: Theme.dimText

                        TextMetrics {
                            id: processIds
                            font: processLine.font
                            text: "  ·  PID " + pane.connection.pid
                                  + (pane.connection.ppid > 0 ? "  ·  PPID " + pane.connection.ppid : "")
                        }
                        TextMetrics {
                            id: processName
                            font: processLine.font
                            text: pane.connection.proc_name
                            elide: Text.ElideRight
                            elideWidth: processLine.width - processIds.advanceWidth
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Theme.gridUnit * 4
                color: Theme.background

                Kirigami.Separator {
                    anchors.bottom: parent.bottom
                    width: parent.width
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: Theme.largeSpacing
                    anchors.rightMargin: Theme.largeSpacing
                    spacing: 0

                    // No rate without the daemon, as the last one would look live.
                    Rate {
                        label: "DOWNLOAD"
                        value: WyreModel.connected && pane.connection.recv_b > 0
                               ? pane.connection.recv + "/s" : "—"
                        total: pane.connection.recv_total
                        color: Theme.download
                    }
                    Kirigami.Separator {
                        Layout.fillHeight: true
                        Layout.topMargin: Theme.largeSpacing
                        Layout.bottomMargin: Theme.largeSpacing
                    }
                    Item {
                        Layout.preferredWidth: Theme.largeSpacing
                    }
                    Rate {
                        label: "UPLOAD"
                        value: WyreModel.connected && pane.connection.send_b > 0
                               ? pane.connection.send + "/s" : "—"
                        total: pane.connection.send_total
                        color: Theme.upload
                    }
                }
            }

            QQC2.ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

                Column {
                    width: Theme.paneWidth

                    DetailField {
                        visible: pane.connection.proto === "TCP"
                        label: "STATE"
                        value: Theme.stateName(pane.connection.state)
                        valueColor: Theme.stateColor(pane.connection.state)
                    }
                    DetailField {
                        label: "REMOTE ADDRESS"
                        value: pane.connection.remote
                        valueColor: Theme.address
                        mono: true
                    }
                    DetailField {
                        visible: pane.location !== ""
                        label: "LOCATION"
                        value: pane.location
                        flag: pane.country === ""
                              ? "" : "qrc:/qt/qml/dev/skepy/wyre/flags/" + pane.country.toLowerCase() + ".png"
                    }
                    DetailField {
                        label: "LOCAL ADDRESS"
                        value: pane.connection.local
                        mono: true
                    }
                }
            }
        }
    }

    // Read-only text that can be selected and copied.
    component SelectableText: TextEdit {
        readOnly: true
        selectByMouse: true
        selectionColor: Theme.accent
        font.family: Theme.monoFamily
    }

    // A rate in big figures, over the connection's lifetime total.
    component Rate: Column {
        id: rate

        property string label
        property string value
        property string total
        property color color

        Layout.fillWidth: true
        Layout.preferredWidth: Theme.gridUnit * 8
        spacing: Theme.smallSpacing * 0.75

        Text {
            text: rate.label
            font.family: Theme.monoFamily
            font.pointSize: Theme.tinySize
            font.letterSpacing: 1.5 * Theme.zoom
            font.bold: true
            color: Theme.dimText
        }
        Text {
            text: rate.value
            font.family: Theme.monoFamily
            font.pointSize: Theme.hugeSize
            font.bold: true
            color: rate.color
        }
        Text {
            text: rate.total + " total"
            font.family: Theme.monoFamily
            font.pointSize: Theme.smallSize
            color: Theme.dimText
        }
    }

    component DetailField: Item {
        id: field

        required property string label
        required property string value
        property color valueColor: Theme.text
        property bool mono: false
        // Shown before the value.
        property url flag

        width: parent.width
        height: column.implicitHeight + Theme.largeSpacing * 2.5

        Kirigami.Separator {
            anchors.bottom: parent.bottom
            width: parent.width
            opacity: 0.6
        }

        Column {
            id: column
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Theme.largeSpacing
            anchors.rightMargin: Theme.largeSpacing
            spacing: Theme.smallSpacing

            Text {
                text: field.label
                font.family: Theme.monoFamily
                font.pointSize: Theme.tinySize
                font.letterSpacing: 1.5 * Theme.zoom
                font.bold: true
                color: Theme.dimText
            }
            Row {
                width: parent.width
                spacing: Theme.smallSpacing

                // Cap height (about 4/3 of the x-height), centred on the first
                // line, and loaded at its device-pixel size so it isn't
                // blurred by scaling. Hidden if there's no flag.
                Image {
                    id: flagImage
                    y: Math.round((valueMetrics.height - height) / 2)
                    visible: status === Image.Ready
                    height: Math.round(valueMetrics.xHeight * 4 / 3)
                    width: Math.round(height * 4 / 3) // the images are 4:3
                    sourceSize.width: Math.round(width * Screen.devicePixelRatio)
                    sourceSize.height: Math.round(height * Screen.devicePixelRatio)
                    source: field.flag

                    // A border for flags with white edges.
                    Rectangle {
                        anchors.fill: parent
                        color: "transparent"
                        border.width: 1
                        border.color: Theme.line
                    }
                }
                // Wrapped, to be read and copied whole.
                SelectableText {
                    id: valueText
                    width: parent.width - (flagImage.visible ? flagImage.width + parent.spacing : 0)
                    wrapMode: Text.Wrap
                    text: field.value
                    font.family: field.mono ? Theme.monoFamily : Kirigami.Theme.defaultFont.family
                    font.pointSize: Theme.mediumSize
                    color: field.valueColor
                }
                FontMetrics {
                    id: valueMetrics
                    font: valueText.font
                }
            }
        }
    }
}
