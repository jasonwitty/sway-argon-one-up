import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import qs.Commons as Commons
import qs.Ui

// Full-screen share card for the connected Wi-Fi network: a QR code any phone
// camera can scan to join, plus the password on request. A leaner take on
// Omarchy's wifiqr panel (shell/plugins/panels/wifiqr), driven by the same
// helper output: `network-panel-qr --meta [iface]` prints a
// "meta\t<iface>\t<security>\t<ssid>" line and then a square 0/1 matrix.
Scope {
  id: root

  property color foreground: Commons.Color.foreground
  property color urgent: Commons.Color.urgent
  property string fontFamily: Style.font.family

  property bool opened: false
  property string iface: ""
  property string ssid: ""
  property bool secured: false
  property var rows: []
  property string error: ""
  property bool loading: false
  property string password: ""
  property bool passwordVisible: false

  readonly property int size: rows.length

  function show(requestedIface) {
    rows = []
    error = ""
    ssid = ""
    iface = ""
    secured = false
    password = ""
    passwordVisible = false
    loading = true
    opened = true
    qrProc.command = requestedIface ? ["network-panel-qr", "--meta", requestedIface] : ["network-panel-qr", "--meta"]
    qrProc.running = true
  }

  function close() {
    opened = false
    // The secret does not outlive the card.
    password = ""
    passwordVisible = false
    rows = []
  }

  function parse(raw) {
    var lines = String(raw || "").trim().split(/\r?\n/).filter(function(line) { return line !== "" })
    if (lines.length > 0 && lines[0].indexOf("meta\t") === 0) {
      var fields = lines.shift().split("\t")
      iface = fields[1] || ""
      secured = !!fields[2] && fields[2] !== "nopass"
      ssid = fields.slice(3).join("\t")
    }
    // A malformed matrix shows the error rather than a code that cannot scan.
    for (var i = 0; i < lines.length; i++) {
      if (lines[i].length !== lines.length || !/^[01]+$/.test(lines[i])) return
    }
    rows = lines
  }

  function togglePassword() {
    if (passwordVisible) { passwordVisible = false; return }
    if (password !== "") { passwordVisible = true; return }
    if (pwProc.running || !iface) return
    pwProc.command = ["network-panel-qr", "--password", iface]
    pwProc.running = true
  }

  Process {
    id: qrProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.parse(text)
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.error = String(text || "").trim()
    }
    onExited: function(exitCode) {
      root.loading = false
      if (exitCode !== 0 || root.size === 0) {
        root.rows = []
        if (root.error === "") root.error = I18n.t("wifiqr.generateFailed")
      } else {
        root.error = ""
      }
    }
  }

  Process {
    id: pwProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (root.opened) root.password = String(text || "").trim()
    }
    onExited: function(exitCode) {
      if (!root.opened) return
      if (exitCode === 0 && root.password !== "") root.passwordVisible = true
      else root.error = I18n.t("wifiqr.passwordFailed")
    }
  }

  PanelWindow {
    visible: root.opened
    color: "transparent"
    exclusionMode: ExclusionMode.Ignore
    anchors { top: true; bottom: true; left: true; right: true }

    WlrLayershell.namespace: "network-panel-qr"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: root.opened ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    onVisibleChanged: if (visible) Qt.callLater(function() { keys.forceActiveFocus() })

    Rectangle {
      anchors.fill: parent
      color: Util.alpha(Commons.Color.background, 0.82)

      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
        onClicked: root.close()
      }
    }

    Item {
      id: keys
      anchors.fill: parent
      focus: true
      Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape || event.text === "q") root.close()
        else if (event.text === "p" && root.secured) root.togglePassword()
        else return
        event.accepted = true
      }
    }

    BorderSurface {
      anchors.centerIn: parent
      width: content.implicitWidth + contentLeftInset + contentRightInset
      height: content.implicitHeight + contentTopInset + contentBottomInset
      color: Commons.Color.popups.background
      borderSpec: Border.surfaceSpec("popups", "border", Commons.Color.popups.border, Math.max(1, Style.space(2)))
      padding: Style.space(24)
      radius: Style.cornerRadius

      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.AllButtons
      }

      ColumnLayout {
        id: content
        anchors.centerIn: parent
        spacing: Style.space(14)

        Text {
          textFormat: Text.PlainText
          Layout.alignment: Qt.AlignHCenter
          Layout.maximumWidth: Style.space(320)
          text: root.ssid || "Wi-Fi"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.heading
          font.bold: true
          elide: Text.ElideRight
        }

        // The code itself is always black on white, whatever the theme: that
        // is what phone cameras expect, quiet zone included.
        Rectangle {
          Layout.alignment: Qt.AlignHCenter
          implicitWidth: Style.space(280)
          implicitHeight: Style.space(280)
          color: "white"
          radius: Math.min(Style.cornerRadius, 6)

          Canvas {
            id: canvas
            anchors.fill: parent
            visible: root.size > 0
            property var rows: root.rows
            onRowsChanged: requestPaint()
            onPaint: {
              var ctx = getContext("2d")
              ctx.clearRect(0, 0, width, height)
              var n = rows.length
              if (n === 0) return
              var cell = Math.floor(Math.min(width, height) / n)
              var ox = Math.floor((width - cell * n) / 2)
              var oy = Math.floor((height - cell * n) / 2)
              // The half-pixel overlap closes the hairline seams fractional
              // output scaling otherwise leaves between modules.
              ctx.fillStyle = "black"
              for (var y = 0; y < n; y++) {
                var line = rows[y]
                for (var x = 0; x < n; x++) {
                  if (line.charAt(x) === "1") ctx.fillRect(ox + x * cell, oy + y * cell, cell + 0.5, cell + 0.5)
                }
              }
            }
          }

          Text {
            textFormat: Text.PlainText
            anchors.centerIn: parent
            width: parent.width - Style.space(24)
            visible: root.size === 0
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: root.loading ? I18n.t("wifiqr.generating") : root.error
            color: root.loading ? "#404040" : "#b00020"
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        Text {
          textFormat: Text.PlainText
          Layout.alignment: Qt.AlignHCenter
          text: I18n.t("wifiqr.scanToJoin")
          color: root.foreground
          opacity: 0.6
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        Button {
          Layout.alignment: Qt.AlignHCenter
          visible: root.secured && root.size > 0
          text: root.passwordVisible ? root.password : I18n.t("wifiqr.showPassword")
          foreground: root.foreground
          fontFamily: root.fontFamily
          bordered: true
          onClicked: root.togglePassword()
        }
      }
    }
  }
}
