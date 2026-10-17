import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import dev.skepy.wyre

// Asked on first launch: whether to look up connection locations, and with
// which databases.
QQC2.Dialog {
    id: setup

    // After choosing imported databases, to import some.
    signal importRequested()

    title: "Show Connection Locations?"
    modal: true
    anchors.centerIn: parent
    width: 440 * Theme.zoom
    visible: WyreModel.geo_choice_needed

    contentItem: ColumnLayout {
        spacing: Theme.largeSpacing

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            font.pointSize: Theme.defaultSize
            text: "Wyre can show the geolocation of IP addresses with a database. This can be changed later from File > Geolocation."
        }
        Choice {
            text: "No Geolocation"
            description: "Don't look up locations at all. Nothing is downloaded."
            mode: "Off"
        }
        Choice {
            text: "Free Databases"
            description: "Country and asn, from <a href=\"https://github.com/sapics/ip-location-db\">ip-location-db</a> (user-country, origin-asn). ~18 MB, updated daily."
            mode: "Free"
        }
        Choice {
            text: "DB-IP Databases"
            description: "Country, asn, and city, from DB-IP's free tier. Also hosted on <a href=\"https://github.com/sapics/ip-location-db\">ip-location-db</a>. ~150 MB total, updated monthly."
            mode: "DbIp"
        }
        Choice {
            text: "Imported Databases"
            description: "Your own MMDB files. Nothing is downloaded."
            mode: "Imported"
            onClicked: setup.importRequested()
        }
    }

    // A button that picks `mode`, described underneath.
    component Choice: ColumnLayout {
        id: choice

        property alias text: button.text
        property string description
        required property string mode

        signal clicked()

        Layout.fillWidth: true
        spacing: 2 * Theme.zoom

        QQC2.Button {
            id: button
            Layout.fillWidth: true
            onClicked: {
                WyreModel.select_geo_mode(choice.mode)
                choice.clicked()
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            opacity: 0.7
            font.pointSize: Theme.smallSize
            text: choice.description
            onLinkActivated: link => Qt.openUrlExternally(link)

            HoverHandler {
                cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor
            }
        }
    }
}
