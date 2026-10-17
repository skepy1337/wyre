import QtQuick
import QtQuick.Layouts
import dev.skepy.wyre

// A connection. Clicking toggles it in the detail pane.
TreeRow {
    id: connectionRow

    height: Theme.gridUnit * 1.3
    backgroundColor: listIndex % 2 === 0 ? Theme.connectionStripe : Theme.background
    selected: Rows.keyOf(row) === Rows.selectedKey
    onClicked: Rows.selected = selected ? null : row

    // Tree lines from the process to this row.
    Rectangle {
        x: Theme.indent + Theme.groupChevronWidth / 2
        width: 1
        height: parent.height
        color: Theme.line
    }
    Rectangle {
        x: Theme.indent + Theme.groupChevronWidth / 2
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.groupChevronWidth / 2 + Theme.processChevronWidth
        height: 1
        color: Theme.line
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.indent + Theme.groupChevronWidth + Theme.processChevronWidth
                            + Theme.largeSpacing
        anchors.rightMargin: Theme.smallSpacing
        spacing: 0

        Item {
            Layout.preferredWidth: Theme.gridUnit * 2.75
            Layout.fillHeight: true

            ProtocolBadge {
                anchors.centerIn: parent
                height: Theme.gridUnit * 0.75
                padding: Theme.largeSpacing / 2
                pointSize: Theme.tinySize
                protocol: connectionRow.row.proto
            }
        }
        Text {
            Layout.fillWidth: true
            leftPadding: Theme.smallSpacing
            text: connectionRow.row.remote
            font.family: Theme.monoFamily
            font.pointSize: Theme.smallSize
            color: Theme.address
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        // Only TCP has states.
        Text {
            visible: connectionRow.row.proto === "TCP"
            leftPadding: Theme.smallSpacing
            text: Theme.stateName(connectionRow.row.state)
            font.family: Theme.monoFamily
            font.pointSize: Theme.tinySize
            color: Theme.stateColor(connectionRow.row.state)
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        NumberCell {
            readonly property bool idle: connectionRow.row.recv_b === 0

            columnWidth: Theme.rateWidth
            pointSize: Theme.smallSize
            text: idle ? "—" : connectionRow.row.recv + "/s"
            color: idle ? Theme.dimText : Theme.download
            bold: !idle
        }
        NumberCell {
            readonly property bool idle: connectionRow.row.send_b === 0

            columnWidth: Theme.rateWidth
            pointSize: Theme.smallSize
            text: idle ? "—" : connectionRow.row.send + "/s"
            color: idle ? Theme.dimText : Theme.upload
            bold: !idle
        }
        // In place of the CONNS column, so the rates line up.
        Item {
            Layout.preferredWidth: Theme.countWidth
            Layout.minimumWidth: Theme.countWidth
        }
    }
}
