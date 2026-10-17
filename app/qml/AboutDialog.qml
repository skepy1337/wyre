import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import dev.skepy.wyre

QQC2.Dialog {
    id: dialog
    
    modal: true
    standardButtons: QQC2.Dialog.Close
    anchors.centerIn: parent
    width: 460 * Theme.zoom

    contentItem: ColumnLayout {
        QQC2.Label {
            text: "Wyre v" + Qt.application.version
            font.pointSize: Theme.largeSize
            font.bold: true
        }

        Heading {
            text: "LOCATION DATA"
        }
        Paragraph {
            text: "IP Geolocation by <a href=\"https://db-ip.com\">DB-IP</a>, hosted on <a href=\"https://github.com/sapics/ip-location-db\">ip-location-db</a>"
        }

        Heading {
            text: "BUILD"
        }
        Paragraph {
            text: "Daemon functionality inside flatpak with <a href=\"https://github.com/ilya-zlobintsev/flatbox\">Flatbox</a> by Ilya Zlobintsev\n
            Rust-Qt linking with <a href=\"https://github.com/KDAB/cxx-qt\">cxx-qt</a>"
        }

        Heading {
            text: "FLAGS"
        }
        Paragraph {
            text: "<a href=\"https://github.com/lipis/flag-icons\">flag-icons</a> by Panayiotis Lipiridis"
        }
    }

    component Paragraph: QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        font.pointSize: Theme.defaultSize
        textFormat: Text.StyledText
        linkColor: Kirigami.Theme.linkColor
        onLinkActivated: link => Qt.openUrlExternally(link)

        HoverHandler {
            cursorShape: parent.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor
        }
    }

    // Like the detail pane's field labels
    component Heading: QQC2.Label {
        Layout.topMargin: Theme.largeSpacing
        font.family: Theme.monoFamily
        font.pointSize: Theme.tinySize
        font.letterSpacing: 1.5 * Theme.zoom
        font.bold: true
        color: Theme.dimText
    }
}
