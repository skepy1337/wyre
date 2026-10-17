import QtQuick
import QtQuick.Layouts
import dev.skepy.wyre

// A program, with its processes' totals. Clicking expands or collapses it.
TreeRow {
    id: groupRow

    height: Theme.gridUnit * 1.6
    backgroundColor: row.exp ? Theme.raisedBackground : Theme.alternateBackground
    separatorOpacity: 1
    onClicked: Rows.changeView(() => WyreModel.toggle_group(row.name))

    // Accent stripe, brighter while expanded.
    Rectangle {
        width: 2
        height: parent.height
        color: Theme.accent
        opacity: groupRow.row.exp ? 1 : 0.25
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Theme.indent
        anchors.rightMargin: Theme.smallSpacing
        spacing: 0

        Chevron {
            size: Theme.groupChevronWidth
            iconSize: Theme.smallIconSize / 1.25
            expanded: groupRow.row.exp
        }
        Text {
            Layout.fillWidth: true
            text: groupRow.row.name
            font.family: Theme.monoFamily
            font.pointSize: Theme.mediumSize
            font.bold: true
            font.capitalization: Font.Capitalize
            color: Theme.text
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        NumberCell {
            columnWidth: Theme.rateWidth
            text: groupRow.row.recv + "/s"
            color: Theme.download
            bold: true
        }
        NumberCell {
            columnWidth: Theme.rateWidth
            text: groupRow.row.send + "/s"
            color: Theme.upload
            bold: true
        }
        NumberCell {
            columnWidth: Theme.countWidth
            text: groupRow.row.conns
            color: Theme.dimText
        }
    }
}
