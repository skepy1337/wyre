pragma Singleton
import QtQuick
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// Colors, fonts and sizes, derived from the user's Kirigami theme. The fonts
// and sizes are zoomed, so use these rather than Kirigami's.
QtObject {
    readonly property color background: Kirigami.Theme.backgroundColor
    readonly property color alternateBackground: Kirigami.Theme.alternateBackgroundColor
    readonly property color raisedBackground: raised(background, 0.06)
    readonly property color headerBackground: raised(background, 0.10)
    readonly property color connectionStripe: raised(background, 0.02)
    readonly property color closedBadge: raised(background, 0.08)

    readonly property color text: Kirigami.Theme.textColor
    readonly property color dimText: Kirigami.Theme.disabledTextColor
    readonly property color address: Kirigami.Theme.linkColor
    readonly property color accent: Kirigami.Theme.highlightColor
    readonly property color positive: Kirigami.Theme.positiveTextColor
    readonly property color neutral: Kirigami.Theme.neutralTextColor
    readonly property color negative: Kirigami.Theme.negativeTextColor
    readonly property color download: positive
    readonly property color upload: neutral

    readonly property color line: translucent(text, 0.12)
    readonly property color strongLine: translucent(text, 0.22)
    readonly property color hover: translucent(text, 0.10)
    readonly property color selection: translucent(accent, 0.18)
    readonly property color tcpBadge: translucent(accent, 0.15)
    readonly property color udpBadge: translucent(positive, 0.15)

    // What every font and size here is multiplied by, from View > Zoom In and
    // Zoom Out.
    readonly property real zoom: WyreModel.zoom

    // For addresses, rates and other data.
    readonly property string monoFamily: Kirigami.Theme.fixedWidthFont.family
    readonly property real tinySize: Math.max(7, Kirigami.Theme.defaultFont.pointSize - 3) * zoom
    readonly property real smallSize: Math.max(8, Kirigami.Theme.defaultFont.pointSize - 2) * zoom
    readonly property real normalSize: (Kirigami.Theme.defaultFont.pointSize - 1) * zoom
    readonly property real mediumSize: (Kirigami.Theme.defaultFont.pointSize + 1) * zoom
    readonly property real largeSize: (Kirigami.Theme.defaultFont.pointSize + 3) * zoom
    readonly property real hugeSize: (Kirigami.Theme.defaultFont.pointSize + 6) * zoom
    // For the standard controls (menus, buttons, labels). The window passes it
    // on to them, but labels must set it themselves, as some styles give them
    // the unzoomed font.
    readonly property real defaultSize: Kirigami.Theme.defaultFont.pointSize * zoom

    // Kirigami's units, rounded to whole pixels like them.
    readonly property int gridUnit: Math.round(Kirigami.Units.gridUnit * zoom)
    readonly property int smallSpacing: Math.round(Kirigami.Units.smallSpacing * zoom)
    readonly property int largeSpacing: Math.round(Kirigami.Units.largeSpacing * zoom)
    readonly property real cornerRadius: Kirigami.Units.cornerRadius * zoom
    readonly property int smallIconSize: Math.round(Kirigami.Units.iconSizes.small * zoom)

    // Widths of the connection list's columns, and of the detail pane.
    readonly property int indent: smallSpacing
    readonly property int groupChevronWidth: gridUnit
    readonly property int processChevronWidth: gridUnit * 0.8
    readonly property int rateWidth: gridUnit * 6
    readonly property int countWidth: gridUnit * 2.75
    readonly property int paneWidth: gridUnit * 16

    // What a gone row fades to before it's removed.
    readonly property real goneOpacity: 0.4

    // `base` tinted toward the text color, so it reads as raised in both
    // light and dark schemes.
    function raised(base, amount) {
        return Qt.tint(base, translucent(text, amount))
    }

    function translucent(color, alpha) {
        return Qt.rgba(color.r, color.g, color.b, alpha)
    }

    function protocolColor(protocol) {
        return protocol === "TCP" ? accent : positive
    }

    function protocolBackground(protocol) {
        return protocol === "TCP" ? tcpBadge : udpBadge
    }

    // TCP states, numbered as in include/net/tcp_states.h.
    readonly property var stateNames: [
        "", "ESTABLISHED", "SYN_SENT", "SYN_RECV", "FIN_WAIT1", "FIN_WAIT2",
        "TIME_WAIT", "CLOSE", "CLOSE_WAIT", "LAST_ACK", "LISTEN", "CLOSING"
    ]

    function stateName(state) {
        if (state === null || state === undefined)
            return "—"
        return stateNames[state] || "UNKNOWN"
    }

    function stateColor(state) {
        if (state === 1)
            return positive // ESTABLISHED
        if (state === 10)
            return accent // LISTEN
        if (state === 6 || state === 11)
            return neutral // TIME_WAIT, CLOSING
        return dimText
    }
}
