import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

// The UI (qml) for this program is completely ai generated
Kirigami.ApplicationWindow {
    title: "Wyre"
    // Qt scales everything by the desktop's scale, so these sizes grow with
    // it, but they're kept within the screen, so the window fits a small or
    // highly scaled one. The minimum grows with the zoom, so the zoomed
    // layout has room; zooming in widens a window that's too small for it.
    width: fit(64, Screen.width)
    height: fit(45, Screen.height)
    minimumWidth: fit(48 * Theme.zoom, Screen.width)
    minimumHeight: fit(28 * Theme.zoom, Screen.height)
    background: Rectangle { color: Theme.background }
    // Passed on to the menus, buttons and other standard controls.
    font.pointSize: Theme.defaultSize

    // `units` grid units, but at most 85% of the screen's `available` length,
    // leaving room for panels.
    function fit(units, available) {
        const size = Kirigami.Units.gridUnit * units
        return available > 0 ? Math.min(size, Math.floor(available * 0.85)) : size
    }

    Timer {
        interval: 1000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: WyreModel.fetch()
    }

    header: Toolbar {
        onImportRequested: importDialog.open()
        onAboutRequested: aboutDialog.open()
    }

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.initialPage: Kirigami.Page {
        padding: 0
        background: Rectangle { color: Theme.background }

        RowLayout {
            anchors.fill: parent
            spacing: 0

            ConnectionList {
                Layout.fillWidth: true
                Layout.fillHeight: true
            }
            Kirigami.Separator {
                Layout.fillHeight: true
            }
            DetailPane {
                Layout.fillHeight: true
            }
        }

        // Ctrl+scroll zooms. On top of the list and pane, so it gets the
        // scroll before they do; without Ctrl, it passes the scroll on.
        Item {
            anchors.fill: parent

            WheelHandler {
                // Touchpads scroll a little at a time, so it's added up to a
                // wheel notch (120) per step.
                property real notches: 0

                target: null
                acceptedModifiers: Qt.ControlModifier
                onWheel: event => {
                    notches += event.angleDelta.y / 120
                    for (; notches >= 1; notches--)
                        WyreModel.zoom_in()
                    for (; notches <= -1; notches++)
                        WyreModel.zoom_out()
                }
            }
        }
    }

    GeoSetupDialog {
        onImportRequested: importDialog.open()
    }

    ImportedDatabasesDialog {
        id: importDialog
    }

    AboutDialog {
        id: aboutDialog
    }
}
