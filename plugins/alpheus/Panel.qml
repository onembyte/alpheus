import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "alpheus"
  ipcTarget: "alpheus"
  manageIpc: true

  property string freeFormatted: "…"
  property string totalFormatted: "…"
  property real freePct: 100
  property real usedPct: 0
  property string reclaimableFormatted: "0 MB"
  property string safeFormatted: "0 MB"
  property var statusData: null
  property bool isScanning: statusProc.running || cleanProc.running

  readonly property var disk: statusData ? statusData.disk : null
  readonly property var summary: statusData ? statusData.summary : null
  readonly property var cards: statusData && statusData.cards ? statusData.cards : []

  function refresh() {
    if (!statusProc.running) {
      statusProc.running = true
    }
  }

  function runCleanSafe() {
    if (!cleanProc.running) {
      cleanProc.running = true
    }
  }

  function openTerminal() {
    if (root.bar) root.bar.run("xdg-terminal-exec -e alpheus -i")
    else Quickshell.execDetached(["xdg-terminal-exec", "-e", "alpheus", "-i"])
    root.close()
  }

  function openBrowse() {
    if (root.bar) root.bar.run("xdg-terminal-exec -e alpheus browse ~")
    else Quickshell.execDetached(["xdg-terminal-exec", "-e", "alpheus", "browse", "~"])
    root.close()
  }

  visible: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Process {
    id: statusProc
    running: false
    command: ["alpheus", "status", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var data = JSON.parse(text)
          root.statusData = data
          if (data && data.disk) {
            root.freeFormatted = data.disk.free_formatted || "…"
            root.totalFormatted = data.disk.total_formatted || "…"
            root.freePct = data.disk.free_pct || 100
            root.usedPct = 100.0 - root.freePct
          }
          if (data && data.summary) {
            root.reclaimableFormatted = data.summary.reclaimable_formatted || "0 MB"
            root.safeFormatted = data.summary.safe_formatted || "0 MB"
          }
        } catch (e) {}
      }
    }
  }

  Process {
    id: cleanProc
    running: false
    command: ["alpheus", "clean", "--all-safe", "-y"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.refresh()
      }
    }
  }

  Timer {
    interval: 60000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  // ---------- Bar Button ----------
  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "💾 " + root.freeFormatted
    tooltipText: "Alpheus Storage: " + root.freeFormatted + " free (" + root.reclaimableFormatted + " reclaimable)"
    horizontalMargin: 7.5
    onPressed: function(b) {
      if (b === Qt.RightButton) {
        root.openTerminal()
      } else {
        root.toggle()
      }
    }
  }

  // ---------- Dropdown Popup Panel ----------
  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(12)

        // Header / Disk Info
        Row {
          width: parent.width
          spacing: Style.space(12)

          Text {
            text: "💾"
            font.pixelSize: Style.font.title
            anchors.verticalCenter: parent.verticalCenter
          }

          Column {
            width: parent.width - Style.space(140)
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(2)

            Text {
              text: "Alpheus Storage"
              color: Color.foreground
              font.pixelSize: Style.font.title
              font.bold: true
            }

            Text {
              text: root.freeFormatted + " free of " + root.totalFormatted + " (" + root.freePct.toFixed(1) + "% available)"
              color: Color.muted
              font.pixelSize: Style.font.bodySmall
            }
          }

          Text {
            text: root.usedPct.toFixed(0) + "%"
            color: Color.foreground
            font.pixelSize: Style.font.displayLarge
            font.bold: true
            anchors.verticalCenter: parent.verticalCenter
          }
        }

        // Progress bar
        Item {
          width: parent.width
          implicitHeight: Style.space(6)

          Rectangle {
            anchors.fill: parent
            radius: height / 2
            color: Util.alpha(Color.foreground, 0.15)
          }

          Rectangle {
            height: parent.height
            width: Math.max(0, Math.min(parent.width, parent.width * (root.usedPct / 100.0)))
            radius: height / 2
            color: root.freePct < 15 ? Color.urgent : Color.accent
          }
        }

        PanelSeparator {}

        // Reclaimable Summary Section
        Row {
          width: parent.width
          spacing: Style.space(8)

          Column {
            width: parent.width - Style.space(120)
            spacing: Style.space(2)

            Text {
              text: "RECLAIMABLE CACHES"
              color: Color.muted
              font.pixelSize: Style.font.caption
              font.bold: true
              font.letterSpacing: 1.0
            }

            Text {
              text: root.safeFormatted + " safe (" + root.reclaimableFormatted + " total)"
              color: Color.accent
              font.pixelSize: Style.font.body
              font.bold: true
            }
          }

          Button {
            text: root.isScanning ? "Cleaning…" : "Clean Safe"
            enabled: !root.isScanning
            bordered: true
            active: true
            anchors.verticalCenter: parent.verticalCenter
            onClicked: root.runCleanSafe()
          }
        }

        // Top 3 Cards Preview
        Repeater {
          model: root.cards.slice(0, 3)
          delegate: Row {
            required property var modelData
            width: parent.width
            spacing: Style.space(8)

            Text {
              text: "• " + (modelData.title || modelData.id)
              color: Color.foreground
              opacity: 0.85
              font.pixelSize: Style.font.bodySmall
              elide: Text.ElideRight
              width: parent.width - Style.space(90)
            }

            Text {
              text: (modelData.size_kb ? ((modelData.size_kb / 1024).toFixed(0) + " MB") : "—")
              color: modelData.tier === "safe" ? Color.accent : Color.urgent
              font.pixelSize: Style.font.bodySmall
              font.bold: true
              horizontalAlignment: Text.AlignRight
              width: Style.space(80)
            }
          }
        }

        PanelSeparator {}

        // Bottom Actions
        Row {
          width: parent.width
          spacing: Style.space(8)

          Button {
            width: (parent.width - Style.space(8)) / 2
            text: "Tree Explorer"
            bordered: true
            onClicked: root.openBrowse()
          }

          Button {
            width: (parent.width - Style.space(8)) / 2
            text: "Interactive TUI"
            bordered: true
            onClicked: root.openTerminal()
          }
        }
      }
    }
  }
}
