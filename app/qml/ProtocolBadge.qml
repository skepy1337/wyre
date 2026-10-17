import QtQuick
import dev.skepy.wyre

// "TCP", "UDP" or "ICMP" on a pill in the protocol's color.
Rectangle {
    id: badge

    required property string protocol
    property real pointSize
    // Horizontal, on each side.
    property real padding

    implicitWidth: label.implicitWidth + padding * 2
    radius: Theme.cornerRadius
    color: Theme.protocolBackground(protocol)

    Text {
        id: label
        anchors.centerIn: parent
        text: badge.protocol
        font.family: Theme.monoFamily
        font.pointSize: badge.pointSize
        font.bold: true
        color: Theme.protocolColor(badge.protocol)
    }
}
