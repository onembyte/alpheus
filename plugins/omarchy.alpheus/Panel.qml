import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "omarchy.alpheus"
  ipcTarget: "omarchy.alpheus"
  manageIpc: false

  property var hostWidget: null
  property var anchorItem: null
  property var statusData: null
  property bool isScanning: false
  readonly property var barIdentity: hostWidget || root

  readonly property var disk: statusData ? statusData.disk : null
  readonly property var summary: statusData ? statusData.summary : null
  readonly property var cards: statusData && statusData.cards ? statusData.cards : []

  readonly property string freeFormatted: disk ? (disk.free_formatted || "…") : "…"
  readonly property string totalFormatted: disk ? (disk.total_formatted || "…") : "…"
  readonly property real freePct: disk ? (disk.free_pct || 0) : 0
  readonly property real usedPct: 100.0 - freePct
  readonly property string reclaimableFormatted: summary ? (summary.reclaimable_formatted || "0 MB") : "0 MB"
  readonly property string safeFormatted: summary ? (summary.safe_formatted || "0 MB") : "0 MB"

  function open() {
    root.controller.show()
    if (hostWidget && hostWidget.refresh) hostWidget.refresh()
  }

  function close() {
    root.controller.hide()
  }

  function toggle() {
    if (root.opened) root.close()
    else root.open()
  }

  function runCleanSafe() {
    if (hostWidget && hostWidget.cleanProc) {
      hostWidget.cleanProc.running = true
    } else {
      Quickshell.execDetached(["alpheus", "clean", "--all-safe", "-y"])
    }
  }

  function openTerminal() {
    Quickshell.execDetached(["omarchy-terminal-window", "alpheus", "scan"])
  }

  contentWidth: Style.space(340)
  contentHeight: panel.fittedContentHeight(column.implicitHeight)

  PanelKeyCatcher {
    id: keyCatcher
    anchors.fill: parent
    onCloseRequested: root.close()

    Column {
      id: column
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      spacing: Style.space(12)

      // ---------------- Header / Hero ----------------
      Row {
        width: parent.width
        spacing: Style.space(12)

        Text {
          text: "󰋊"
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.display
          color: root.freePct < 10.0 ? (root.bar ? root.bar.urgent : Color.urgent) : (root.bar ? root.bar.foreground : Color.foreground)
          anchors.verticalCenter: parent.verticalCenter
        }

        Column {
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(2)

          Text {
            text: "Alpheus Storage"
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.title
            font.bold: true
            color: root.bar ? root.bar.foreground : Color.foreground
          }

          Text {
            text: root.freeFormatted + " free of " + root.totalFormatted + " (" + root.freePct.toFixed(1) + "% available)"
            font.family: root.bar ? root.bar.fontFamily : Style.font.family
            font.pixelSize: Style.font.caption
            color: Qt.darker(root.bar ? root.bar.foreground : Color.foreground, 1.35)
          }
        }
      }

      // ---------------- Disk Usage Bar ----------------
      Rectangle {
        width: parent.width
        height: Style.space(8)
        radius: Style.space(4)
        color: Qt.darker(root.bar ? root.bar.foreground : Color.foreground, 3.5)

        Rectangle {
          width: Math.max(Style.space(4), (parent.width * (root.usedPct / 100.0)))
          height: parent.height
          radius: parent.radius
          color: root.usedPct > 90.0 ? (root.bar ? root.bar.urgent : Color.urgent) : (root.bar ? root.bar.foreground : Color.foreground)
        }
      }

      PanelSeparator {
        width: parent.width
        bar: root.bar
      }

      // ---------------- Reclaimable Summary ----------------
      Row {
        width: parent.width
        Text {
          text: "Reclaimable:"
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.body
          color: root.bar ? root.bar.foreground : Color.foreground
        }

        Item { Layout.fillWidth: true; width: 1 }

        Text {
          text: root.reclaimableFormatted + " (" + root.safeFormatted + " safe)"
          font.family: root.bar ? root.bar.fontFamily : Style.font.family
          font.pixelSize: Style.font.body
          font.bold: true
          color: (root.summary && root.summary.reclaimable_kb > 0) ? "#4ade80" : (root.bar ? root.bar.foreground : Color.foreground)
        }
      }

      // ---------------- Category Cards List ----------------
      ListView {
        id: cardsList
        width: parent.width
        implicitHeight: Math.min(220, contentHeight)
        clip: true
        model: root.cards

        delegate: Item {
          width: cardsList.width
          implicitHeight: Style.space(34)

          Row {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Style.space(8)

            Rectangle {
              width: Style.space(6)
              height: Style.space(6)
              radius: Style.space(3)
              anchors.verticalCenter: parent.verticalCenter
              color: modelData.tier === "safe" ? "#4ade80" : (modelData.tier === "with-care" ? "#facc15" : "#94a3b8")
            }

            Column {
              width: parent.width - sizeText.width - Style.space(24)
              spacing: 0

              Text {
                text: modelData.title || modelData.id
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.caption
                font.bold: true
                color: root.bar ? root.bar.foreground : Color.foreground
                elide: Text.ElideRight
                width: parent.width
              }

              Text {
                text: modelData.action === "command" ? (modelData.command_display || "command") : (modelData.paths && modelData.paths.length > 0 ? modelData.paths[0] : "—")
                font.family: root.bar ? root.bar.fontFamily : Style.font.family
                font.pixelSize: Style.font.caption - 1
                color: Qt.darker(root.bar ? root.bar.foreground : Color.foreground, 1.5)
                elide: Text.ElideMiddle
                width: parent.width
              }
            }

            Text {
              id: sizeText
              text: {
                var kb = modelData.size_kb || 0
                var gb = kb / (1024 * 1024)
                if (gb >= 1.0) return gb.toFixed(1) + " GB"
                var mb = kb / 1024
                if (mb >= 1.0) return mb.toFixed(0) + " MB"
                return kb + " KB"
              }
              font.family: root.bar ? root.bar.fontFamily : Style.font.family
              font.pixelSize: Style.font.caption
              font.bold: true
              color: root.bar ? root.bar.foreground : Color.foreground
              anchors.verticalCenter: parent.verticalCenter
            }
          }
        }
      }

      PanelSeparator {
        width: parent.width
        bar: root.bar
      }

      // ---------------- Actions ----------------
      Row {
        width: parent.width
        spacing: Style.space(8)

        Button {
          text: root.isScanning ? "Cleaning…" : "Clean Safe (" + root.safeFormatted + ")"
          enabled: !root.isScanning && root.summary && root.summary.safe_kb > 0
          bar: root.bar
          onClicked: root.runCleanSafe()
        }

        Button {
          text: "Terminal"
          bar: root.bar
          onClicked: root.openTerminal()
        }

        Button {
          text: "↻"
          bar: root.bar
          onClicked: {
            if (hostWidget && hostWidget.refresh) hostWidget.refresh()
          }
        }
      }
    }
  }
}
