import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "omarchy.alpheus"

  property string freeFormatted: "…"
  property string totalFormatted: "…"
  property real freePct: 100
  property string reclaimableFormatted: "0 MB"
  property var statusData: null
  property bool isScanning: statusProc.running || cleanProc.running
  property string lastCleanMessage: ""

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
    if ("statusData" in target) target.statusData = root.statusData
    if ("isScanning" in target) target.isScanning = root.isScanning
  }

  function refresh() {
    if (!statusProc.running) {
      statusProc.running = true
    }
  }

  function togglePanel() {
    if (panelLoader.item) {
      if (panelLoader.item.opened) panelLoader.item.close()
      else panelLoader.item.open()
    }
  }

  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false

  function open() {
    if (panelLoader.item && panelLoader.item.open) panelLoader.item.open()
  }

  function close() {
    if (panelLoader.item && panelLoader.item.close) panelLoader.item.close()
  }

  visible: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()
  onStatusDataChanged: {
    if (panelLoader.item) panelLoader.item.statusData = root.statusData
  }
  onIsScanningChanged: {
    if (panelLoader.item) panelLoader.item.isScanning = root.isScanning
  }

  Process {
    id: statusProc
    running: false
    command: ["alpheus", "status", "--json"]
    stdout: StdioCollector {
      id: stdoutCollector
      waitForEnd: true
      onStreamFinished: {
        try {
          var data = JSON.parse(text)
          root.statusData = data
          if (data && data.disk) {
            root.freeFormatted = data.disk.free_formatted || "…"
            root.totalFormatted = data.disk.total_formatted || "…"
            root.freePct = data.disk.free_pct || 100
          }
          if (data && data.summary) {
            root.reclaimableFormatted = data.summary.reclaimable_formatted || "0 MB"
          }
        } catch (e) {
          // ignore parse errors during incomplete stream
        }
      }
    }
    onExited: function(code) {
      root.injectPanel()
    }
  }

  Process {
    id: cleanProc
    running: false
    command: ["alpheus", "clean", "--all-safe", "-y"]
    stdout: StdioCollector {
      id: cleanStdout
      waitForEnd: true
      onStreamFinished: {
        root.lastCleanMessage = text
      }
    }
    onExited: function(code) {
      root.refresh()
    }
  }

  Timer {
    id: pollTimer
    interval: 60000 // Poll every 60s
    repeat: true
    running: true
    onTriggered: root.refresh()
  }

  Component.onCompleted: {
    root.refresh()
  }

  WidgetButton {
    id: button
    bar: root.bar
    text: "󰋊 " + root.freeFormatted
    tooltipText: "Alpheus Storage: " + root.freeFormatted + " free (" + root.reclaimableFormatted + " reclaimable)"
    active: root.opened
    foreground: root.freePct < 10.0 ? (bar ? bar.urgent : Color.urgent) : (bar ? bar.barForeground : Color.foreground)

    onPressed: function(btn) {
      if (btn === Qt.RightButton) {
        Quickshell.execDetached(["alpheus", "scan"])
      } else {
        root.togglePanel()
      }
    }
  }

  Loader {
    id: panelLoader
    active: true
    source: "Panel.qml"
    onLoaded: root.injectPanel()
  }
}
