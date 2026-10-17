import QtQuick
import QtQuick.Layouts
import dev.skepy.wyre

// A right-aligned figure in one of the number columns. Keeps exactly the
// column's width, even when a long address beside it runs out of room, so
// figures line up under their headers.
Item {
    id: cell

    required property real columnWidth
    property string text
    property color color: Theme.text
    property bool bold: false
    property real pointSize: Theme.normalSize
    // Space kept clear at the start, for a control placed there.
    property real reserved: 0

    Layout.preferredWidth: columnWidth
    Layout.minimumWidth: columnWidth
    Layout.fillHeight: true
    clip: true

    Text {
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Math.max(Theme.largeSpacing, cell.reserved)
        anchors.rightMargin: Theme.largeSpacing
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
        text: cell.text
        font.family: Theme.monoFamily
        font.pointSize: cell.pointSize
        font.bold: cell.bold
        color: cell.color
    }
}
