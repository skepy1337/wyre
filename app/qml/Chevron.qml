import QtQuick
import QtQuick.Layouts
import QtQuick.Shapes
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// The arrow on a group or process row: pointing down while it's expanded.
// Drawn as a path, so it stays sharp at any size and while it turns.
Item {
    id: chevron

    required property bool expanded
    // Of the square the arrow sits in, and of the arrow itself.
    required property real size
    required property real iconSize
    property real lineWidth: Math.max(1.25, iconSize / 7)
    property color color: Theme.text

    Layout.preferredWidth: size
    Layout.preferredHeight: size

    Shape {
        id: arrow

        // Half the arrow's height, and its width, so it turns about its middle.
        readonly property real half: chevron.iconSize * 0.3
        readonly property real middle: chevron.iconSize / 2

        anchors.centerIn: parent
        width: chevron.iconSize
        height: chevron.iconSize
        preferredRendererType: Shape.CurveRenderer
        rotation: chevron.expanded ? 90 : 0
        Behavior on rotation { NumberAnimation { duration: Kirigami.Units.shortDuration; easing.type: Easing.OutCubic } }

        ShapePath {
            strokeColor: chevron.color
            strokeWidth: chevron.lineWidth
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            startX: arrow.middle - arrow.half / 2
            startY: arrow.middle - arrow.half
            PathLine { x: arrow.middle + arrow.half / 2; y: arrow.middle }
            PathLine { x: arrow.middle - arrow.half / 2; y: arrow.middle + arrow.half }
        }
    }

    Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
}
