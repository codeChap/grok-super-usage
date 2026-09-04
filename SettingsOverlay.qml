import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Commons
import qs.Ui

// Centered settings card. The bar panel is too small to paste a key into.
PanelWindow {
  id: root

  property bool opened: false
  property color foreground: Color.menu.text
  property color dim: Color.muted
  property string fontFamily: Style.font.family
  property bool showWeeklyUsage: true
  property bool showApiBilling: true
  property bool paceAlarmEnabled: false
  property string managementKeyPath: ""
  property bool billingHasData: false
  property string billingLabel: ""
  property string billingHelpText: ""

  signal closed()
  signal flagChanged(string key, bool on)
  signal keyPathCommitted(string path)

  visible: opened
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  anchors { top: true; bottom: true; left: true; right: true }

  WlrLayershell.namespace: "codechap-grok-super-usage-settings"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: opened ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

  onOpenedChanged: {
    if (opened) Qt.callLater(function() {
      if (root.opened) keyCatcher.forceActiveFocus()
    })
  }

  Shortcut {
    sequence: "Escape"
    enabled: root.opened
    onActivated: root.closed()
  }

  Rectangle {
    anchors.fill: parent
    color: Color.menu.scrim
  }

  MouseArea {
    anchors.fill: parent
    onClicked: root.closed()
  }

  BorderSurface {
    id: card
    width: Math.min(Style.space(520), parent.width - Style.gapsOut * 2)
    height: card.contentTopInset + card.contentBottomInset + column.implicitHeight
    anchors.centerIn: parent
    color: Color.menu.background
    borderSpec: Border.surfaceSpec("menu", "border", Color.menu.border, Math.max(1, Style.space(2)))
    padding: Style.spacing.panelPadding
    radius: Style.cornerRadius

    MouseArea { anchors.fill: parent; onClicked: {} }

    Item {
      id: keyCatcher
      anchors.fill: parent
      anchors.topMargin: card.contentTopInset
      anchors.rightMargin: card.contentRightInset
      anchors.bottomMargin: card.contentBottomInset
      anchors.leftMargin: card.contentLeftInset
      focus: true

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        Text {
          width: parent.width
          text: "Settings"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.title
          font.bold: true
        }

        Text {
          width: parent.width
          text: "api billing"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        SettingsForm {
          width: parent.width
          foreground: root.foreground
          dim: root.dim
          fontFamily: root.fontFamily
          showWeeklyUsage: root.showWeeklyUsage
          showApiBilling: root.showApiBilling
          paceAlarmEnabled: root.paceAlarmEnabled
          managementKeyPath: root.managementKeyPath
          billingHasData: root.billingHasData
          billingLabel: root.billingLabel
          billingHelpText: root.billingHelpText
          onFlagChanged: function(key, on) { root.flagChanged(key, on) }
          onKeyPathCommitted: function(path) { root.keyPathCommitted(path) }
        }

        Text {
          width: parent.width
          text: "Esc close"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
    }
  }
}
