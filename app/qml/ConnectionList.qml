pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import Qt.labs.qmlmodels
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

ColumnLayout {
    spacing: 0

    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: Theme.gridUnit * 1.5
        color: Theme.headerBackground

        Kirigami.Separator {
            anchors.bottom: parent.bottom
            width: parent.width
        }

        // Lined up with the rows' columns.
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.indent
            anchors.rightMargin: Theme.smallSpacing
            spacing: 0

            Item {
                Layout.preferredWidth: Theme.groupChevronWidth + Theme.processChevronWidth
            }
            ColumnHeader {
                Layout.fillWidth: true
                label: "APPLICATION"
                sortKey: "name"
            }
            ColumnHeader {
                Layout.preferredWidth: Theme.rateWidth
                label: "DOWNLOAD"
                sortKey: "download"
            }
            ColumnHeader {
                Layout.preferredWidth: Theme.rateWidth
                label: "UPLOAD"
                sortKey: "upload"
            }
            ColumnHeader {
                Layout.preferredWidth: Theme.countWidth
                label: "CONNS"
                sortKey: "conns"
                centered: true
            }
        }
    }

    ListView {
        id: list

        // Returns memory to the system once the list shrinks (by 100 rows
        // from its peak, or with 5 delegates pooled). Three things hold on to
        // it, each releasable only after the one before: pooled delegates,
        // uncollected JS garbage, and malloc's free lists. So the pool is
        // emptied, then garbage collected, then malloc trimmed.
        //
        // A big process's connections re-sorting cycles delegates through
        // the pool every second, so a reused delegate postpones a trim that
        // is only about the pool.
        property int peakRows: 0
        property int pooledDelegates: 0
        readonly property bool shrunk: peakRows - count >= 100
        readonly property bool worthTrimming: shrunk || pooledDelegates >= 5

        function delegatePooled() {
            pooledDelegates++
            trimSoon()
        }

        function delegateReused() {
            pooledDelegates--
            if (trimTimer.running && !shrunk)
                trimTimer.restart()
        }

        function trimSoon() {
            if (worthTrimming)
                trimTimer.start()
        }

        Layout.fillWidth: true
        Layout.fillHeight: true
        model: Rows.model
        clip: true
        reuseItems: true
        cacheBuffer: 400
        boundsBehavior: Flickable.StopAtBounds
        QQC2.ScrollBar.vertical: QQC2.ScrollBar { policy: QQC2.ScrollBar.AsNeeded }

        delegate: DelegateChooser {
            role: "kind"

            DelegateChoice {
                roleValue: "G"
                GroupRow {
                    ListView.onPooled: list.delegatePooled()
                    ListView.onReused: list.delegateReused()
                }
            }
            DelegateChoice {
                roleValue: "P"
                ProcessRow {
                    ListView.onPooled: list.delegatePooled()
                    ListView.onReused: list.delegateReused()
                }
            }
            DelegateChoice {
                roleValue: "C"
                ConnectionRow {
                    ListView.onPooled: list.delegatePooled()
                    ListView.onReused: list.delegateReused()
                }
            }
        }

        Column {
            anchors.centerIn: parent
            visible: list.count === 0
            spacing: Theme.largeSpacing

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: WyreModel.connected ? "No active connections"
                    : WyreModel.error === "" ? "Connecting to daemon…"
                    : "Cannot reach daemon"
                font.family: Theme.monoFamily
                font.pointSize: Theme.largeSize
                font.bold: true
                color: Theme.dimText
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: WyreModel.connected ? "Traffic will appear here as it is detected."
                    : WyreModel.error === "" ? "Waiting for the first response…"
                    : WyreModel.error
                font.family: Theme.monoFamily
                font.pointSize: Theme.normalSize
                color: Theme.dimText
                opacity: 0.55
            }
        }

        Connections {
            target: Rows
            function onUpdated() {
                // Measured from the peak, so rows going a few at a time add up.
                list.peakRows = Math.max(list.peakRows, list.count)
                list.trimSoon()
            }
        }

        Timer {
            id: trimTimer
            interval: 1500
            onTriggered: {
                if (list.moving) {
                    restart() // not in the middle of a flick
                    return
                }
                // The rows may have come back in the meantime.
                if (list.worthTrimming) {
                    // Turning reuse off destroys the pooled delegates.
                    list.reuseItems = false
                    list.reuseItems = true
                    list.pooledDelegates = 0
                    collectTimer.start()
                }
                list.peakRows = list.count
            }
        }

        // A moment later, once the destroyed delegates are deleted.
        Timer {
            id: collectTimer
            interval: 200
            onTriggered: {
                // Since Qt 6.8 the collector is incremental: the first gc()
                // finishes a cycle that may predate the rows going, the
                // second runs a full one.
                gc()
                gc()
                WyreModel.trim_memory()
            }
        }
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: Theme.gridUnit * 1.2
        color: Theme.headerBackground

        Kirigami.Separator {
            anchors.top: parent.top
            width: parent.width
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.largeSpacing
            anchors.rightMargin: Theme.largeSpacing

            Text {
                text: WyreModel.total_conns + " connection(s)"
                font.family: Theme.monoFamily
                font.pointSize: Theme.normalSize
                color: Theme.dimText
            }
            Item {
                Layout.fillWidth: true
            }
            Text {
                visible: !WyreModel.connected
                text: WyreModel.error
                font.family: Theme.monoFamily
                font.pointSize: Theme.normalSize
                color: Theme.negative
                elide: Text.ElideLeft
            }
        }
    }

    // Clicking sorts the programs by this column, or reverses the sort.
    component ColumnHeader: Item {
        id: header

        required property string label
        required property string sortKey
        property bool centered: false
        readonly property bool active: WyreModel.sort_by === sortKey

        Layout.fillHeight: true

        Rectangle {
            x: title.x - Theme.smallSpacing
            y: Theme.smallSpacing / 2
            width: title.width + Theme.smallSpacing * 2
            height: parent.height - Theme.smallSpacing
            radius: Theme.smallSpacing * 0.75
            color: Theme.hover
            opacity: mouseArea.containsMouse ? 1 : 0
            Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
        }

        Row {
            id: title
            x: header.centered ? (parent.width - width) / 2 : Theme.smallSpacing
            height: parent.height
            spacing: Theme.smallSpacing

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: header.label
                font.family: Theme.monoFamily
                font.pointSize: Theme.smallSize
                font.letterSpacing: 1.6 * Theme.zoom
                font.bold: true
                color: Theme.dimText
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: header.active
                text: WyreModel.sort_desc ? "▼" : "▲"
                font.family: Theme.monoFamily
                font.pointSize: Theme.tinySize
                color: Theme.accent
            }
        }

        MouseArea {
            id: mouseArea
            x: title.x - Theme.smallSpacing
            width: title.width + Theme.smallSpacing * 2
            height: parent.height
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            // A new column starts with the biggest first, or names from A.
            onClicked: WyreModel.set_sort(
                header.sortKey, header.active ? !WyreModel.sort_desc : header.sortKey !== "name")
        }
    }
}
