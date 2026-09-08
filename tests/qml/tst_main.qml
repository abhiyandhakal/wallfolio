import QtQuick
import QtTest
import "../../gui/qml" as App

Item {
    width: 1180
    height: 780
    App.Main {
        id: app
        client: mock
        TestCase {
            id: test
            name: "WallfolioClient"
            when: windowShown
            width: 1180
            height: 780
            QtObject {
                id: mock
                property bool busy: false
                property var calls: []
                property int starts: 0
                signal completed(string method, var result)
                signal failed(string message)
                signal unavailable(string message)
                function request(method, params) {
                    calls = calls.concat([
                        {
                            method: method,
                            params: params
                        }
                    ]);
                }
                function fileUrl(path) {
                    return "";
                }
                function startDaemon() {
                    starts++;
                }
            }
            Component {
                id: windowFactory
                App.Main {
                    client: mock
                }
            }
            function locate(item, name) {
                if (item.objectName === name)
                    return item;
                var children = item.children || [];
                for (var i = 0; i < children.length; ++i) {
                    var found = locate(children[i], name);
                    if (found)
                        return found;
                }
                if (item.contentItem && item.contentItem !== item)
                    return locate(item.contentItem, name);
                return null;
            }
            function click(item) {
                verify(item !== null);
                mouseClick(item, item.width / 2, item.height / 2);
            }
            function init() {
                wait(100);
                mock.calls = [];
                mock.starts = 0;
                findChild(app, "detailDialog").close();
                app.settingsLoaded = true;
                app.backendOptions = [{id:"swww",label:"swww",available:true,per_monitor:true}, {id:"hyprpaper",label:"hyprpaper",available:true,per_monitor:true}];
                app.preferredBackend = "swww";
                app.syncBackendSelection();
                findChild(app, "rotationDialog").close();
                app.duplicateGroups = [];
                app.cacheAttempts = 120;
                app.page = 1;
                app.startupRetry = false;
                app.selected = null;
                app.section = "Library";
                app.wallpapers = [];
                mock.busy = false;
            }
            function test_pageJumpAndValidation() {
                app.section = "Discover";
                var input = findChild(app, "pageInput");
                input.text = "37";
                input.forceActiveFocus();
                keyClick(Qt.Key_Return);
                compare(app.page, 37);
                compare(mock.calls.length, 1);
                compare(mock.calls[0].method, "provider.search");
                compare(mock.calls[0].params.page, 37);
                input.text = "0";
                verify(!findChild(app, "pageGo").enabled);
                app.jumpToPage();
                compare(mock.calls.length, 1);
                input.text = "1.5";
                app.jumpToPage();
                compare(mock.calls.length, 1);
                input.text = "92";
                click(findChild(app, "pageGo"));
                compare(mock.calls[1].params.page, 92);
            }
            function test_engineRestoredForNewWindow() {
                mock.calls = [];
                var reopened = windowFactory.createObject(test);
                compare(mock.calls[0].method, "device.settings");
                mock.completed("device.settings", {
                    preferred_backend: "hyprpaper"
                });
                compare(mock.calls[1].method, "device.backends");
                mock.completed("device.backends", app.backendOptions);
                compare(findChild(reopened, "backendSelector").currentText, "hyprpaper");
                compare(mock.calls[2].method, "catalog.search");
                reopened.destroy();
            }
            function test_engineSelectionIsSavedImmediately() {
                app.selected = {
                    id: "saved",
                    title: "Saved image",
                    local_path: "managed.png",
                    provider: "local",
                    source: "",
                    tags: []
                };
                var dialog = findChild(app, "detailDialog");
                dialog.open();
                tryCompare(dialog, "opened", true);
                var selector = findChild(app, "backendSelector");
                verify(waitForRendering(selector));
                selector.currentIndex = 0;
                click(selector);
                keyClick(Qt.Key_End);
                keyClick(Qt.Key_Return);
                compare(mock.calls.length, 1);
                compare(mock.calls[0].method, "device.settings.update");
                compare(mock.calls[0].params.preferred_backend, "hyprpaper");
                dialog.close();
            }
            function test_savedDiscoveryCardUpdatesWithoutRefetch() {
                app.section = "Discover";
                app.wallpapers = [
                    {
                        title: "Mountains",
                        provider: "wallhaven",
                        external_id: "abc123",
                        source: "",
                        thumbnail: null,
                        tags: []
                    }
                ];
                mock.completed("catalog.add", {
                    id: "catalog-123",
                    title: "Mountains",
                    provider: "wallhaven",
                    external_id: "abc123",
                    source: "",
                    tags: [],
                    favorite: false,
                    local_path: null
                });
                compare(app.wallpapers[0].id, "catalog-123");
                compare(mock.calls.length, 0);
                mock.completed("wallpaper.download", {
                    id: "catalog-123",
                    title: "Mountains",
                    provider: "wallhaven",
                    external_id: "abc123",
                    source: "",
                    tags: [],
                    favorite: true,
                    local_path: "managed.png"
                });
                compare(app.wallpapers[0].local_path, "managed.png");
                wait(100);
                mouseClick(findChild(app, "wallpaperGrid"), 60, 60);
                var dialog = findChild(app, "detailDialog");
                tryCompare(dialog, "opened", true);
                verify(!findChild(app, "saveButton").visible);
                compare(findChild(app, "downloadButton").text, "Downloaded");
                verify(!findChild(app, "downloadButton").enabled);
                compare(app.selected.favorite, true);
                mock.completed("wallpaper.delete_local", {
                    id: "catalog-123",
                    title: "Mountains",
                    provider: "wallhaven",
                    external_id: "abc123",
                    source: "",
                    tags: [],
                    favorite: true,
                    local_path: null
                });
                compare(app.wallpapers[0].local_path, null);
                verify(findChild(app, "downloadButton").enabled);
                dialog.close();
            }
            function test_favoritesAndDiscoveryUseCore() {
                click(locate(app, "navFavorites"));
                compare(app.section, "Favorites");
                compare(mock.calls[0].method, "catalog.search");
                compare(mock.calls[0].params.favorite, true);
                click(locate(app, "navDiscover"));
                compare(mock.calls[1].method, "provider.search");
                compare(mock.calls[1].params.provider, "wallhaven");
            }
            function test_randomAndRotationUseCore() {
                click(locate(app, "rotationButton"));
                compare(mock.calls[0].method, "rotation.status");
                var dialog = findChild(app, "rotationDialog");
                tryCompare(dialog, "opened", true);
                mock.completed("rotation.status", {enabled:true, interval_seconds:1800, favorite:true, tags:["dark"], monitor:"DP-1", next_run:2000000000});
                compare(findChild(app, "rotationInterval").text, "1800");
                compare(findChild(app, "rotationTags").text, "dark");
                verify(findChild(app, "rotationFavorite").checked);
                findChild(app, "rotationInterval").text = "5";
                verify(!findChild(app, "rotationStart").enabled);
                findChild(app, "rotationInterval").text = "60";
                findChild(app, "rotationTags").text = "dark, landscape, ";
                mock.calls = [];
                click(findChild(app, "randomNow"));
                compare(mock.calls[0].method, "wallpaper.random");
                compare(mock.calls[0].params.tags, ["dark", "landscape"]);
                compare(mock.calls[0].params.favorite, true);
                compare(mock.calls[0].params.monitor, "DP-1");
                compare(mock.calls[0].params.backend, "swww");
                click(findChild(app, "rotationStart"));
                compare(mock.calls[1].method, "rotation.configure");
                compare(mock.calls[1].params.interval_seconds, 60);
                grabImage(app.contentItem).save("/tmp/wallfolio-rotation-controls.png");
                click(findChild(app, "rotationStop"));
                compare(mock.calls[2].method, "rotation.stop");
                dialog.close();
            }
            function test_backendCapabilitiesAndUnavailablePreference() {
                app.preferredBackend = "gnome";
                mock.completed("device.backends", [{id:"swww",available:true,per_monitor:true}, {id:"gnome",available:false,per_monitor:false}]);
                compare(findChild(app, "backendSelector").currentText, "gnome (unavailable)");
                compare(findChild(app, "rotationBackend").currentIndex, 1);
                verify(!findChild(app, "rotationMonitor").enabled);
                compare(app.rotationParams().monitor, "");
            }
            function test_duplicatesAndThumbnailLookup() {
                click(locate(app, "navDuplicates"));
                compare(mock.calls[0].method, "catalog.duplicates");
                mock.completed("catalog.duplicates", [{count:2,content_hash:"same",items:[{id:"one",title:"One",thumbnail_key:"key",provider:"local",source:""}]}]);
                compare(app.duplicateGroups[0].count, 2);
                app.lookupThumbnails();
                compare(mock.calls[1].method, "cache.lookup");
                compare(mock.calls[1].params.keys, ["key"]);
                mock.completed("cache.lookup", {key:"/cache/thumb.png"});
                compare(app.duplicateGroups[0].items[0].cached_thumbnail, "/cache/thumb.png");
                compare(mock.calls.length, 2);
            }
            function test_providerFailureDoesNotRestartDaemon() {
                mock.failed("Provider unavailable");
                compare(mock.starts, 0);
                compare(app.status, "Provider unavailable");
            }
            function test_missingDaemonRetriesOnce() {
                mock.unavailable("Missing daemon");
                mock.unavailable("Still missing");
                compare(mock.starts, 1);
                wait(850);
                compare(mock.calls.length, 1);
                mock.completed("catalog.search", []);
                mock.unavailable("Daemon stopped later");
                compare(mock.starts, 2);
                wait(850);
            }
            function test_previewAndDownloadKeepCatalogId() {
                app.wallpapers = [
                    {
                        id: "catalog-123",
                        title: "Mountains",
                        provider: "wallhaven",
                        external_id: "abc123",
                        source: "",
                        thumbnail: null,
                        tags: [],
                        favorite: false,
                        local_path: null
                    }
                ];
                wait(100);
                mouseClick(findChild(app, "wallpaperGrid"), 60, 60);
                var dialog = findChild(app, "detailDialog");
                tryCompare(dialog, "opened", true);
                var button = findChild(app, "downloadButton");
                verify(waitForRendering(button));
                click(button);
                compare(mock.calls[0].method, "wallpaper.download");
                compare(mock.calls[0].params.id, "catalog-123");
                dialog.close();
            }
        }
    }
}
