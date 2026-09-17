import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "custom.cyberwatch"

  readonly property string pluginDir: decodeURIComponent(
    String(Qt.resolvedUrl("."))
      .replace(/^file:\/\//, "")
      .replace(/\/+$/, ""))
  readonly property string togglePath: pluginDir + "/cyberwatch-toggle"

  property int attention: 0
  property int total: 0
  property bool haveData: false

  function launch() {
    if (!root.bar) return
    root.bar.run(Util.shellQuote(root.togglePath))
  }

  function parseSummary(raw) {
    try {
      var parsed = JSON.parse(raw)
      if (parsed && typeof parsed.attention === "number") {
        root.attention = parsed.attention
        root.total = parsed.total || 0
        root.haveData = true
      }
    } catch (e) {
      // Leave the last-known good values on screen rather than blanking the
      // widget over one bad poll (a mid-rescan hiccup, a transient error).
    }
  }

  function refresh() {
    if (!summaryProc.running) summaryProc.running = true
  }

  Component.onCompleted: refresh()

  Timer {
    interval: 60000
    running: true
    repeat: true
    onTriggered: root.refresh()
  }

  Process {
    id: summaryProc
    command: [root.togglePath, "--summary"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.parseSummary(text)
    }
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    bar: root.bar
    text: root.haveData ? (" " + root.attention) : ""
    tooltipText: root.haveData
      ? (root.attention + " of " + root.total + " units need attention — click to open cyberwatch")
      : "cyberwatch — scanning…"
    active: root.haveData && root.attention > 0
    activeColor: Color.urgent
    horizontalMargin: 6
    verticalPadding: 6
    fixedWidth: root.vertical ? root.barSize : Style.space(28)
    fixedHeight: root.barSize
    onPressed: root.launch()
  }
}
