import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// A process, with its totals. Clicking expands or collapses it; while it's
// expanded and hovered, its rate cells offer to sort connections by rate.
TreeRow {
    id: processRow

    readonly property bool sortTogglesShown: hovered && row.exp

    height: Theme.gridUnit * 1.5
    backgroundColor: listIndex % 2 === 0 ? Theme.background : Theme.alternateBackground
    onClicked: Rows.changeView(() => WyreModel.toggle_proc(row.pid))

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.indent
        anchors.rightMargin: Theme.smallSpacing
        spacing: 0

        Chevron {
            size: Theme.processChevronWidth
            iconSize: Theme.smallIconSize / 1.5
            color: processRow.row.exp ? Theme.text : Theme.dimText
            expanded: processRow.row.exp
        }
        Item {
            Layout.preferredWidth: Theme.smallSpacing * 2
        }
        // The program's row above already shows the name.
        Text {
            Layout.fillWidth: true
            text: "Process " + processRow.row.pid
            font.family: Theme.monoFamily
            font.pointSize: Theme.normalSize
            color: Theme.text
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        NumberCell {
            columnWidth: Theme.rateWidth
            text: processRow.row.recv + "/s"
            color: Theme.download
            reserved: downloadSort.visible ? downloadSort.x + downloadSort.width : 0

            SortToggle {
                id: downloadSort
                visible: processRow.sortTogglesShown
                label: "DL"
                sortKey: "download"
            }
        }
        NumberCell {
            columnWidth: Theme.rateWidth
            text: processRow.row.send + "/s"
            color: Theme.upload
            reserved: uploadSort.visible ? uploadSort.x + uploadSort.width : 0

            SortToggle {
                id: uploadSort
                visible: processRow.sortTogglesShown
                label: "UL"
                sortKey: "upload"
            }
        }
        NumberCell {
            columnWidth: Theme.countWidth
            text: processRow.row.conns
            color: Theme.dimText
        }
    }

    // Sorts every process's connections by one rate, cycling through biggest
    // first, smallest first, and off (by address).
    component SortToggle: Item {
        id: toggle

        required property string label
        required property string sortKey
        readonly property bool active: WyreModel.conn_sort_by === sortKey

        x: Theme.smallSpacing
        y: (parent.height - height) / 2
        width: title.implicitWidth + Theme.smallSpacing
        height: Theme.gridUnit

        Rectangle {
            anchors.fill: parent
            radius: Theme.smallSpacing * 0.75
            color: Theme.hover
            opacity: mouseArea.containsMouse ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
        }

        MouseArea {
            id: mouseArea
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                if (!toggle.active)
                    WyreModel.set_conn_sort(toggle.sortKey, true)
                else if (WyreModel.conn_sort_desc)
                    WyreModel.set_conn_sort(toggle.sortKey, false)
                else
                    WyreModel.set_conn_sort("remote", false)
            }
        }

        Row {
            id: title
            anchors.centerIn: parent
            spacing: Theme.smallSpacing * 0.75

            Text {
                text: toggle.label
                font.family: Theme.monoFamily
                font.pointSize: Theme.smallSize
                font.letterSpacing: 1 * Theme.zoom
                font.bold: toggle.active
                color: Theme.dimText
            }
            // Dimmed while off, as a hint the toggle is there.
            Text {
                text: toggle.active && !WyreModel.conn_sort_desc ? "▲" : "▼"
                font.family: Theme.monoFamily
                font.pointSize: Theme.tinySize
                color: toggle.active ? Theme.accent : Theme.dimText
                opacity: toggle.active ? 1 : 0.7
            }
        }
    }
}
