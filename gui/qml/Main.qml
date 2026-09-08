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
    property bool settingsLoaded: false
    property string preferredBackend: ""
    property var backendOptions: []
    property var backendIds: backendOptions.map(function(b) { return b.id })
    property var duplicateGroups: []
    property var rotation: ({enabled: false, interval_seconds: 1800, tags: [], favorite: false})
    property int cacheAttempts: 0
    function syncBackendSelection() {
        let index = backendIds.indexOf(preferredBackend)
        if (!preferredBackend) index = backendOptions.findIndex(function(b) { return b.available })
        backend.currentIndex = index
        scheduleBackend.currentIndex = index
    }
    function supportsMonitor(index) { return index >= 0 && backendOptions[index].per_monitor }
    function rotationParams() {
        return {enabled: true, interval_seconds: Number(rotationInterval.text),
            favorite: rotationFavorite.checked,
            tags: rotationTags.text.split(",").map(function(t) { return t.trim() }).filter(function(t) { return !!t }),
            monitor: supportsMonitor(scheduleBackend.currentIndex) ? rotationMonitor.text : ""}
    }
    function lookupThumbnails() {
        let items = section === "Duplicates" ? duplicateGroups.reduce(function(all, group) { return all.concat(group.items) }, []) : wallpapers
        let keys = items.filter(function(item) { return item.thumbnail_key && !item.cached_thumbnail }).map(function(item) { return item.thumbnail_key })
        if (keys.length && cacheAttempts++ < 120) client.request("cache.lookup", {keys: keys.slice(0,100)})
    }
    function thumbnailSource(item) {
        return item.cached_thumbnail ? client.fileUrl(item.cached_thumbnail) : (item.local_path ? client.fileUrl(item.local_path) : (item.thumbnail || (item.provider === "local" ? client.fileUrl(item.source) : "")))
    }
    onPageChanged: pageInput.text = String(page)
    function updateWallpaper(result) {
        selected = result
        wallpapers = wallpapers.map(function(item) {
            return item.provider === result.provider && item.external_id === result.external_id ? result : item
        })
    }
    function jumpToPage() {
        if (!client.busy && pageInput.acceptableInput) {
            page = Number(pageInput.text)
            refresh()
        }
    }
    property string status: "Connecting to your library…"
    property bool startupRetry: false
    function refresh() {
        if (!settingsLoaded) { client.request("device.settings", {}); return }
        if (section === "Duplicates") client.request("catalog.duplicates", {limit: 10, offset: (page-1)*10})
        else if (section === "Discover") client.request("provider.search", {provider: provider.currentText, query: search.text, page: page})
        else client.request("catalog.search", {query: search.text, favorite: section === "Favorites", limit: 24, offset: (page-1)*24})
    }
    function action(method, extra) {
        let params = extra || {}; params.id = selected.id
        client.request(method, params)
    }
    Component.onCompleted: refresh()
    Timer { id: reconnect; interval: 800; onTriggered: window.refresh() }
    Timer { interval: 1000; running: window.visible; repeat: true; onTriggered: { if (!client.busy && window.settingsLoaded) window.lookupThumbnails() } }
    Connections {
        target: client
        function onCompleted(method, result) {
            if (method !== "cache.lookup") window.status = "Ready"
            window.startupRetry = false
            if (method === "device.settings" || method === "device.settings.update") {
                window.preferredBackend = result.preferred_backend || ""
                window.syncBackendSelection()
                if (!window.settingsLoaded) client.request("device.backends", {})
            }
            else if (method === "device.backends") {
                window.backendOptions = result.map(function(b) { b.label = b.id + (b.available ? "" : " (unavailable)"); return b })
                window.syncBackendSelection()
                if (!window.settingsLoaded) { window.settingsLoaded = true; window.refresh() }
            }
            else if (method === "catalog.search" || method === "provider.search") { window.wallpapers = result; window.cacheAttempts = 0 }
            else if (method === "catalog.duplicates") { window.duplicateGroups = result; window.cacheAttempts = 0 }
            else if (method === "cache.lookup") {
                function cached(item) {
                    if (!result[item.thumbnail_key]) return item
                    return Object.assign({}, item, {cached_thumbnail: result[item.thumbnail_key]})
                }
                window.wallpapers = window.wallpapers.map(cached)
                window.duplicateGroups = window.duplicateGroups.map(function(g) { return Object.assign({}, g, {items:g.items.map(cached)}) })
            }
            else if (method.indexOf("rotation.") === 0) {
                window.rotation = result
                rotationInterval.text = String(result.interval_seconds)
                rotationTags.text = (result.tags || []).join(", ")
                rotationFavorite.checked = !!result.favorite
                rotationMonitor.text = result.monitor || ""
                window.status = result.enabled ? "Wallpaper rotation enabled" : "Wallpaper rotation stopped"
            }
            else if (method === "wallpaper.random") window.status = "Applied " + result.wallpaper.title
            else if (method === "catalog.add") { window.updateWallpaper(result); window.status = "Saved to your library"; if(window.section !== "Discover") window.refresh() }
            else if (method === "wallpaper.apply") window.status = "Wallpaper applied"
            else if (method === "catalog.remove") { detail.close(); window.refresh() }
            else if (result && result.id) { window.updateWallpaper(result); if(window.section !== "Discover") window.refresh() }
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
                    model: ["Library", "Discover", "Favorites", "Duplicates"]
                    Button {
                        required property string modelData
                        objectName: "nav" + modelData
                        text: modelData; Layout.fillWidth: true; highlighted: window.section === modelData
                        enabled: !client.busy
                        onClicked: { window.section = modelData; window.page = 1; search.text = ""; window.refresh() }
                    }
                }
                Button { objectName: "rotationButton"; text: "Random & rotation"; Layout.fillWidth: true; enabled: !client.busy && window.settingsLoaded
                    onClicked: { rotationDialog.open(); client.request("rotation.status", {}) } }
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
            Label { text: window.section === "Discover" ? "Find something worth coming back to." : (window.section === "Duplicates" ? "Identical files share storage. Your library entries stay separate." : "Your saved wallpapers, independent of where you found them."); color: "#95a6a5" }
            RowLayout {
                visible: window.section !== "Duplicates"
                ComboBox { id: provider; visible: window.section === "Discover"; model: ["wallhaven", "local"]; enabled: !client.busy }
                TextField { id: search; objectName: "searchInput"; Layout.fillWidth: true; placeholderText: window.section === "Discover" && provider.currentText === "local" ? "Absolute folder path" : "Search titles and tags…"; onAccepted: { window.page=1; window.refresh() } }
                Button { text: "Search"; enabled: !client.busy; onClicked: { window.page=1; window.refresh() } }
                Button { text: "Import file"; enabled: !client.busy; onClicked: importDialog.open() }
            }
            GridView {
                visible: window.section !== "Duplicates"
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
                                source: window.thumbnailSource(modelData)
                                onStatusChanged: if (status === Image.Error && modelData.cached_thumbnail) source = modelData.local_path ? client.fileUrl(modelData.local_path) : (modelData.thumbnail || "")
                                sourceSize.width: 480; sourceSize.height: 320
                                asynchronous: true; fillMode: Image.PreserveAspectCrop
                            }
                            Label { text: modelData.title; Layout.fillWidth: true; elide: Text.ElideRight; font.bold: true }
                            Label { text: (modelData.favorite ? "♥  " : "") + modelData.provider + (modelData.local_path ? " · Downloaded" : (modelData.id ? " · In library" : "")); color: "#a0b4af"; font.pixelSize: 12 }
                        }
                        MouseArea { anchors.fill: parent; enabled: !client.busy; onClicked: { window.selected = modelData; detail.open() } }
                    }
                }
                Label { anchors.centerIn: parent; visible: !client.busy && window.wallpapers.length === 0; text: "No wallpapers here yet. Import a file or explore Discover."; color: "#95a6a5"; wrapMode: Text.WordWrap; width: parent.width-40; horizontalAlignment: Text.AlignHCenter }
            }
            ListView {
                objectName: "duplicateList"; visible: window.section === "Duplicates"
                Layout.fillWidth: true; Layout.fillHeight: true; clip: true; spacing: 18
                model: window.duplicateGroups
                ScrollBar.vertical: ScrollBar {}
                delegate: ColumnLayout {
                    required property var modelData
                    width: ListView.view.width
                    Label { text: modelData.count + " identical images"; font.bold: true }
                    Repeater {
                        model: modelData.items
                        Button { required property var modelData; Layout.fillWidth: true
                            text: modelData.title + " · " + modelData.provider + (modelData.local_path ? " · Downloaded" : " · Remote only")
                            enabled: !client.busy
                            onClicked: { window.selected = modelData; detail.open() }
                        }
                    }
                    Label { visible: modelData.count > modelData.items.length; text: "Showing the first " + modelData.items.length + " entries in this group"; color: "#95a6a5" }
                }
                Label { anchors.centerIn: parent; visible: !client.busy && !window.duplicateGroups.length; text: "No duplicate files found on this page."; color: "#95a6a5" }
            }
            RowLayout {
                Button { text: "Previous"; enabled: window.page>1 && !client.busy; onClicked: { window.page--; window.refresh() } }
                Label { text: window.section === "Discover" ? "Page" : "Page " + window.page; color: "#95a6a5" }
                TextField {
                    id: pageInput; objectName: "pageInput"; visible: window.section === "Discover"
                    Layout.preferredWidth: 110; text: "1"; enabled: !client.busy
                    validator: IntValidator { bottom: 1; top: 2147483647 }
                    inputMethodHints: Qt.ImhDigitsOnly
                    onAccepted: window.jumpToPage()
                }
                Button { objectName: "pageGo"; text: "Go"; visible: window.section === "Discover"; enabled: !client.busy && pageInput.acceptableInput; onClicked: window.jumpToPage() }
                Button { text: "Next"; enabled: window.page < 2147483647 && (window.section === "Duplicates" ? window.duplicateGroups.length === 10 : window.wallpapers.length === 24) && !client.busy; onClicked: { window.page++; window.refresh() } }
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
                Button { objectName: "saveButton"; text: "Save to library"; visible: window.selected && !window.selected.id; onClicked: client.request("catalog.add", {provider: window.selected.provider, external_id: window.selected.external_id}) }
                Button { objectName: "downloadButton"; text: window.selected && window.selected.local_path ? "Downloaded" : "Download"; enabled: window.selected && !window.selected.local_path; visible: window.selected && !!window.selected.id; onClicked: window.action("wallpaper.download") }
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
                ComboBox {
                    id: backend; objectName: "backendSelector"; model: window.backendOptions; textRole: "label"
                    onActivated: client.request("device.settings.update", {preferred_backend: window.backendIds[currentIndex]})
                }
                TextField { id: monitor; enabled: window.supportsMonitor(backend.currentIndex); placeholderText: backend.currentIndex >= 0 && window.backendIds[backend.currentIndex] === "nitrogen" ? "Numeric monitor index (blank for all)" : "Monitor (blank for default)"; Layout.fillWidth: true }
                Button { text: "Set wallpaper"; enabled: backend.currentIndex >= 0; onClicked: window.action("wallpaper.apply", {backend: window.backendIds[backend.currentIndex], monitor: monitor.enabled ? monitor.text : ""}) }
            }
            Label { text: window.status; Layout.fillWidth: true; wrapMode: Text.Wrap; color: "#a6d3b4" }
        }
    }
    Dialog {
        id: rotationDialog; objectName: "rotationDialog"; title: "Random wallpaper & rotation"
        anchors.centerIn: parent; width: Math.min(window.width-50, 560); modal: true
        standardButtons: Dialog.Close
        ColumnLayout {
            width: parent.width; spacing: 12
            Label { text: "Choose from downloaded library images. Uses your saved wallpaper engine."; Layout.fillWidth: true; wrapMode: Text.Wrap }
            ComboBox { id: scheduleBackend; objectName: "rotationBackend"; Layout.fillWidth: true
                model: window.backendOptions; textRole: "label"; enabled: !client.busy
                onActivated: client.request("device.settings.update", {preferred_backend: window.backendIds[currentIndex]}) }
            TextField { id: rotationMonitor; objectName: "rotationMonitor"; Layout.fillWidth: true
                enabled: !client.busy && window.supportsMonitor(scheduleBackend.currentIndex)
                placeholderText: window.backendIds[scheduleBackend.currentIndex] === "nitrogen" ? "Numeric monitor index (blank for all)" : "Monitor (blank for default)" }
            CheckBox { id: rotationFavorite; objectName: "rotationFavorite"; text: "Favorites only"; enabled: !client.busy }
            TextField { id: rotationTags; objectName: "rotationTags"; Layout.fillWidth: true; placeholderText: "Required tags, comma separated"; enabled: !client.busy }
            RowLayout {
                Label { text: "Interval (seconds)" }
                TextField { id: rotationInterval; objectName: "rotationInterval"; text: "1800"; Layout.fillWidth: true
                    validator: IntValidator { bottom: 10; top: 604800 }
                    inputMethodHints: Qt.ImhDigitsOnly; enabled: !client.busy }
            }
            RowLayout {
                Button { objectName: "randomNow"; text: "Apply random now"; enabled: !client.busy && scheduleBackend.currentIndex >= 0
                    onClicked: { let params = window.rotationParams(); delete params.interval_seconds; delete params.enabled; params.backend = window.backendIds[scheduleBackend.currentIndex]; client.request("wallpaper.random", params) } }
                Button { objectName: "rotationStart"; text: window.rotation.enabled ? "Update rotation" : "Start rotation"
                    enabled: !client.busy && rotationInterval.acceptableInput && scheduleBackend.currentIndex >= 0
                    onClicked: client.request("rotation.configure", window.rotationParams()) }
                Button { objectName: "rotationStop"; text: "Stop"; enabled: !client.busy && !!window.rotation.enabled; onClicked: client.request("rotation.stop", {}) }
            }
            Label { text: window.rotation.enabled ? "Running · next change " + new Date(window.rotation.next_run * 1000).toLocaleString() : "Rotation is off"; Layout.fillWidth: true; wrapMode: Text.Wrap }
            Label { visible: !!window.rotation.last_error; text: window.rotation.last_error || ""; color: "#efb5a6"; Layout.fillWidth: true; wrapMode: Text.Wrap }
            Label { text: window.status; color: "#a6d3b4"; Layout.fillWidth: true; wrapMode: Text.Wrap }
        }
    }
    Dialog {
        id: removeDialog; anchors.centerIn: parent; modal: true; title: "Remove from your library?"
        standardButtons: Dialog.Yes | Dialog.No
        Label { text: "The catalog entry will be removed. Local files are kept.\nUse Delete local copy first if you also want to free disk space." }
        onAccepted: window.action("catalog.remove")
    }
}
