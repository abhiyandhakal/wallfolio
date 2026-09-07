import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: window
    required property var client
    visible: true
    width: 1180; height: 780
    minimumWidth: 820; minimumHeight: 580
    title: "Wallfolio"
    color: "#111719"
    palette.window: "#111719"
    palette.windowText: "#eaf0eb"
    palette.base: "#1c2528"
    palette.text: "#eaf0eb"
    palette.placeholderText: "#95a6a5"
    palette.highlightedText: "#111719"
    palette.button: "#283437"
    palette.buttonText: "#eaf0eb"
    palette.highlight: "#82b995"
    property string section: "Library"
    property var wallpapers: []
    property var selected: null
    property int page: 1
    property string status: "Connecting to your library…"
    property bool startupRetry: false
    function refresh() {
        if (section === "Discover") client.request("provider.search", {provider: provider.currentText, query: search.text, page: page})
        else client.request("catalog.search", {query: search.text, favorite: section === "Favorites", limit: 24, offset: (page-1)*24})
    }
    function action(method, extra) {
        let params = extra || {}; params.id = selected.id
        client.request(method, params)
    }
    Component.onCompleted: refresh()
    Timer { id: reconnect; interval: 800; onTriggered: window.refresh() }
    Connections {
        target: client
        function onCompleted(method, result) {
            window.status = "Ready"
            window.startupRetry = false
            if (method === "catalog.search" || method === "provider.search") window.wallpapers = result
            else if (method === "catalog.add") { window.selected = result; window.status = "Saved to your library"; if(window.section !== "Discover") window.refresh() }
            else if (method === "wallpaper.apply") window.status = "Wallpaper applied"
            else if (method === "catalog.remove") { detail.close(); window.refresh() }
            else if (result && result.id) { window.selected = result; if(window.section !== "Discover") window.refresh() }
        }
        function onFailed(message) {
            window.status = message
        }
        function onUnavailable(message) {
            window.status = message
            if (!window.startupRetry) { window.startupRetry = true; client.startDaemon(); reconnect.start() }
        }
    }
    RowLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.preferredWidth: 196; Layout.fillHeight: true; color: "#192124"
            ColumnLayout {
                anchors.fill: parent; anchors.margins: 22; spacing: 16
                Label { text: "WALLFOLIO"; font.pixelSize: 19; font.bold: true; color: "#a6d3b4" }
                Label { text: "A place for your walls."; color: "#95a6a5"; font.pixelSize: 12 }
                Item { Layout.preferredHeight: 20 }
                Repeater {
                    model: ["Library", "Discover", "Favorites"]
                    Button {
                        required property string modelData
                        objectName: "nav" + modelData
                        text: modelData; Layout.fillWidth: true; highlighted: window.section === modelData
                        enabled: !client.busy
                        onClicked: { window.section = modelData; window.page = 1; search.text = ""; window.refresh() }
                    }
                }
                Item { Layout.fillHeight: true }
                Label { text: "LOCAL FIRST\nYour catalog, your choice."; color: "#95a6a5"; font.pixelSize: 12; lineHeight: 1.6 }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; Layout.margins: 28; spacing: 18
            RowLayout {
                Label { text: window.section; font.pixelSize: 32; font.bold: true; Layout.fillWidth: true }
                BusyIndicator { running: client.busy; visible: running; Layout.preferredWidth: 32; Layout.preferredHeight: 32 }
            }
            Label { text: window.section === "Discover" ? "Find something worth coming back to." : "Your saved wallpapers, independent of where you found them."; color: "#95a6a5" }
            RowLayout {
                ComboBox { id: provider; visible: window.section === "Discover"; model: ["wallhaven", "local"]; enabled: !client.busy }
                TextField { id: search; objectName: "searchInput"; Layout.fillWidth: true; placeholderText: window.section === "Discover" && provider.currentText === "local" ? "Absolute folder path" : "Search titles and tags…"; onAccepted: { window.page=1; window.refresh() } }
                Button { text: "Search"; enabled: !client.busy; onClicked: { window.page=1; window.refresh() } }
                Button { text: "Import file"; enabled: !client.busy; onClicked: importDialog.open() }
            }
            GridView {
                id: grid
                objectName: "wallpaperGrid"
                Layout.fillWidth: true; Layout.fillHeight: true; clip: true
                cellWidth: Math.floor(width / Math.max(2, Math.floor(width/250))); cellHeight: 206
                model: window.wallpapers
                ScrollBar.vertical: ScrollBar {}
                delegate: Item {
                    required property var modelData
                    width: grid.cellWidth; height: grid.cellHeight
                    Rectangle {
                        anchors.fill: parent; anchors.margins: 6; radius: 10; color: "#202b2e"
                        ColumnLayout {
                            anchors.fill: parent; anchors.margins: 8; spacing: 8
                            Image {
                                Layout.fillWidth: true; Layout.fillHeight: true
                                source: modelData.local_path ? client.fileUrl(modelData.local_path) : (modelData.thumbnail || (modelData.provider === "local" ? client.fileUrl(modelData.source) : ""))
                                sourceSize.width: 480; sourceSize.height: 320
                                asynchronous: true; fillMode: Image.PreserveAspectCrop
                            }
                            Label { text: modelData.title; Layout.fillWidth: true; elide: Text.ElideRight; font.bold: true }
                            Label { text: (modelData.favorite ? "♥  " : "") + modelData.provider + (modelData.local_path ? " · Downloaded" : ""); color: "#a0b4af"; font.pixelSize: 12 }
                        }
                        MouseArea { anchors.fill: parent; enabled: !client.busy; onClicked: { window.selected = modelData; detail.open() } }
                    }
                }
                Label { anchors.centerIn: parent; visible: !client.busy && window.wallpapers.length === 0; text: "No wallpapers here yet. Import a file or explore Discover."; color: "#95a6a5"; wrapMode: Text.WordWrap; width: parent.width-40; horizontalAlignment: Text.AlignHCenter }
            }
            RowLayout {
                Button { text: "Previous"; enabled: window.page>1 && !client.busy; onClicked: { window.page--; window.refresh() } }
                Label { text: "Page " + window.page; color: "#95a6a5" }
                Button { text: "Next"; enabled: window.wallpapers.length === 24 && !client.busy; onClicked: { window.page++; window.refresh() } }
                Item { Layout.fillWidth: true }
            }
            Label { text: window.status; Layout.fillWidth: true; wrapMode: Text.WordWrap; color: "#a6d3b4" }
        }
    }
    Dialog {
        id: importDialog; title: "Import a local image"; anchors.centerIn: parent; width: 520; modal: true
        standardButtons: Dialog.Ok | Dialog.Cancel
        TextField { id: importPath; width: parent.width; placeholderText: "Absolute path to a PNG, JPEG or WebP image" }
        onAccepted: client.request("catalog.add", {provider: "local", external_id: importPath.text})
    }
    Dialog {
        id: detail; objectName: "detailDialog"; anchors.centerIn: parent; width: Math.min(window.width-50,850); height: window.height-60; modal: true
        title: window.selected ? window.selected.title : "Wallpaper"
        standardButtons: Dialog.Close
        ColumnLayout {
            anchors.fill: parent; spacing: 12
            Image {
                Layout.fillWidth: true; Layout.fillHeight: true; asynchronous: true
                source: window.selected ? (window.selected.local_path ? client.fileUrl(window.selected.local_path) : (window.selected.thumbnail || (window.selected.provider === "local" ? client.fileUrl(window.selected.source) : ""))) : ""
                sourceSize.width: 1600; sourceSize.height: 1000; fillMode: Image.PreserveAspectFit
            }
            Label { text: window.selected ? (window.selected.tags || []).join(" · ") : ""; wrapMode: Text.Wrap; Layout.fillWidth: true }
            RowLayout {
                enabled: !client.busy
                Button { text: "Save to library"; visible: window.selected && !window.selected.id; onClicked: client.request("catalog.add", {provider: window.selected.provider, external_id: window.selected.external_id}) }
                Button { objectName: "downloadButton"; text: "Download"; visible: window.selected && !!window.selected.id; onClicked: window.action("wallpaper.download") }
                Button { text: window.selected && window.selected.favorite ? "Unfavorite" : "Favorite"; visible: window.selected && !!window.selected.id; onClicked: window.action(window.selected.favorite ? "favorite.remove" : "favorite.add") }
                Button { text: "Delete local copy"; visible: window.selected && !!window.selected.local_path; onClicked: window.action("wallpaper.delete_local") }
                Button { text: "Remove from library"; visible: window.selected && !!window.selected.id; onClicked: removeDialog.open() }
            }
            RowLayout {
                visible: window.selected && !!window.selected.id; enabled: !client.busy
                TextField { id: tags; Layout.fillWidth: true; placeholderText: "Replace tags (comma separated)" }
                Button { text: "Save tags"; onClicked: window.action("catalog.tags", {tags: tags.text.split(",")}) }
            }
            RowLayout {
                visible: window.selected && !!window.selected.local_path; enabled: !client.busy
                ComboBox { id: backend; model: ["swww", "hyprpaper"] }
                TextField { id: monitor; placeholderText: "Monitor (blank for default)"; Layout.fillWidth: true }
                Button { text: "Set wallpaper"; onClicked: window.action("wallpaper.apply", {backend: backend.currentText, monitor: monitor.text}) }
            }
            Label { text: window.status; Layout.fillWidth: true; wrapMode: Text.Wrap; color: "#a6d3b4" }
        }
    }
    Dialog {
        id: removeDialog; anchors.centerIn: parent; modal: true; title: "Remove from your library?"
        standardButtons: Dialog.Yes | Dialog.No
        Label { text: "The catalog entry will be removed. Local files are kept.\nUse Delete local copy first if you also want to free disk space." }
        onAccepted: window.action("catalog.remove")
    }
}
