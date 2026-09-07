import QtQuick
import QtTest
import "../../gui/qml" as App

TestCase {
    id: test
    name: "WallfolioClient"
    when: windowShown
    width: 1180; height: 780
    QtObject {
        id: mock
        property bool busy: false
        property var calls: []
        property int starts: 0
        signal completed(string method, var result)
        signal failed(string message)
        signal unavailable(string message)
        function request(method, params) { calls = calls.concat([{method:method, params:params}]) }
        function fileUrl(path) { return "file://" + path }
        function startDaemon() { starts++ }
    }
    App.Main { id: app; client: mock }
    function locate(item, name) {
        if (item.objectName === name) return item
        var children = item.children || []
        for (var i=0; i<children.length; ++i) {
            var found = locate(children[i],name)
            if(found) return found
        }
        if(item.contentItem && item.contentItem !== item) return locate(item.contentItem,name)
        return null
    }
    function click(item) {
        verify(item !== null)
        mouseClick(item,item.width/2,item.height/2)
    }
    function init() {
        wait(100)
        mock.calls = []
        mock.starts = 0
        app.startupRetry = false
        app.selected = null
        app.section = "Library"
        app.wallpapers = []
        mock.busy = false
    }
    function test_favoritesAndDiscoveryUseCore() {
        click(locate(app,"navFavorites"))
        compare(app.section, "Favorites")
        compare(mock.calls[0].method, "catalog.search")
        compare(mock.calls[0].params.favorite, true)
        click(locate(app,"navDiscover"))
        compare(mock.calls[1].method, "provider.search")
        compare(mock.calls[1].params.provider, "wallhaven")
    }
    function test_providerFailureDoesNotRestartDaemon() {
        mock.failed("Provider unavailable")
        compare(mock.starts,0)
        compare(app.status,"Provider unavailable")
    }
    function test_missingDaemonRetriesOnce() {
        mock.unavailable("Missing daemon")
        mock.unavailable("Still missing")
        compare(mock.starts,1)
        wait(850)
        compare(mock.calls.length,1)
        mock.completed("catalog.search",[])
        mock.unavailable("Daemon stopped later")
        compare(mock.starts,2)
        wait(850)
    }
    function test_previewAndDownloadKeepCatalogId() {
        app.wallpapers = [{id:"catalog-123",title:"Mountains",provider:"wallhaven",external_id:"abc123",source:"",thumbnail:null,tags:[],favorite:false,local_path:null}]
        wait(100)
        mouseClick(findChild(app,"wallpaperGrid"),60,60)
        var dialog = findChild(app,"detailDialog")
        tryCompare(dialog,"opened",true)
        var button = findChild(app,"downloadButton")
        verify(waitForRendering(button))
        click(button)
        compare(mock.calls[0].method,"wallpaper.download")
        compare(mock.calls[0].params.id,"catalog-123")
        dialog.close()
    }
}
