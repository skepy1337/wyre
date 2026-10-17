pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import dev.skepy.wyre

// The user's own MMDB databases, used instead of the downloaded ones.
QQC2.Dialog {
    id: dialog

    readonly property var databases: JSON.parse(WyreModel.imported_json)
    readonly property bool inUse: WyreModel.geo_mode === "Imported"

    title: "Import Databases"
    modal: true
    standardButtons: QQC2.Dialog.Close
    anchors.centerIn: parent
    width: 480 * Theme.zoom

    contentItem: ColumnLayout {
        spacing: Theme.largeSpacing

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.pointSize: Theme.defaultSize
            text: "Use any MMDB database."
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: Theme.smallSpacing

            Repeater {
                model: dialog.databases

                DatabaseRow {
                    inUse: dialog.inUse
                }
            }
            QQC2.Label {
                visible: dialog.databases.length === 0
                opacity: 0.7
                font.pointSize: Theme.defaultSize
                text: "None imported yet."
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Theme.smallSpacing

            QQC2.Button {
                text: "Import…"
                onClicked: fileDialog.open()
            }
            // Importing switches to them; this switches back.
            QQC2.Button {
                visible: !dialog.inUse && dialog.databases.length > 0
                text: "Use These"
                onClicked: WyreModel.select_geo_mode("Imported")
            }
            Item {
                Layout.fillWidth: true
            }
        }
    }

    FileDialog {
        id: fileDialog
        title: "Import Databases"
        fileMode: FileDialog.OpenFiles
        nameFilters: ["MMDB databases (*.mmdb *.mmdb.gz)", "All files (*)"]
        onAccepted: {
            for (let i = 0; i < selectedFiles.length; i++)
                WyreModel.import_database(selectedFiles[i])
        }
    }

    // One imported database: file name, what it has (or its error), and path.
    component DatabaseRow: RowLayout {
        id: row

        required property var modelData
        required property int index
        property bool inUse

        readonly property string description: {
            if (modelData.error !== "")
                return modelData.error
            if (modelData.loaded)
                return modelData.kind + " — " + (modelData.has !== "" ? modelData.has : "no countries, cities or networks found")
            return inUse ? "Importing…" : "Not loaded while another source is in use"
        }

        Layout.fillWidth: true
        spacing: Theme.largeSpacing

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2 * Theme.zoom

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                font.pointSize: Theme.defaultSize
                font.bold: true
                text: row.modelData.path.split("/").pop()
            }
            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                font.pointSize: Theme.smallSize
                color: row.modelData.error !== "" ? Theme.negative : Theme.text
                opacity: row.modelData.error !== "" ? 1 : 0.7
                text: row.description
            }
            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                font.pointSize: Theme.tinySize
                opacity: 0.6
                text: row.modelData.path
            }
        }
        QQC2.Button {
            text: "Remove"
            onClicked: WyreModel.remove_database(row.index)
        }
    }
}
