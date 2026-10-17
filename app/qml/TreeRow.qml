import QtQuick
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// Base for the connection list's rows: background, hover and selection
// highlight, click, and the gone look (faded, with a grey stripe since
// closing is normal, and unresponsive).
Item {
    id: treeRow

    required property var row
    required property int listIndex
    readonly property bool gone: row.gone === true
    readonly property bool hovered: hoverHandler.hovered && !gone
    property bool selected: false
    property color backgroundColor
    property real separatorOpacity: 0.35
    default property alias content: body.data

    signal clicked()

    width: ListView.view.width

    HoverHandler {
        id: hoverHandler
    }

    MouseArea {
        anchors.fill: parent
        enabled: !treeRow.gone
        cursorShape: Qt.PointingHandCursor
        onClicked: treeRow.clicked()
    }

    // Content goes here, over the background and highlight.
    Item {
        id: body
        anchors.fill: parent
        opacity: treeRow.gone ? Theme.goneOpacity : 1
        Behavior on opacity { NumberAnimation { duration: Kirigami.Units.longDuration } }

        Rectangle {
            anchors.fill: parent
            color: treeRow.backgroundColor
        }
        Rectangle {
            anchors.fill: parent
            visible: !treeRow.gone
            color: treeRow.selected ? Theme.selection : Theme.hover
            opacity: treeRow.hovered || treeRow.selected ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
        }
    }

    Rectangle {
        width: 2
        height: parent.height
        color: treeRow.gone ? Theme.strongLine : Theme.accent
        visible: treeRow.selected || treeRow.gone
    }

    Kirigami.Separator {
        anchors.bottom: parent.bottom
        width: parent.width
        opacity: treeRow.separatorOpacity
    }
}
