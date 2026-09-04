import QtQuick
import qs.Commons
import qs.Ui

Item {
  id: hdr
  property string title: ""
  property string meta: ""
  property real metaOpacity: 0
  property bool settingsOpen: false
  property color foreground: Color.foreground
  property color dim: Color.foreground
  property string fontFamily: Style.font.family
  signal settingsClicked()
  signal consoleClicked()

  implicitHeight: Math.max(iconBox.height, titleCol.implicitHeight, cog.implicitHeight)

  Item {
    id: iconBox
    width: Style.font.display
    height: titleText.height
    anchors.left: parent.left
    anchors.top: parent.top

    GrokMark {
      anchors.centerIn: parent
      width: Style.font.display
      height: Style.font.display
      color: "#ffffff"
    }
  }

  Column {
    id: titleCol
    anchors.left: iconBox.right
    anchors.leftMargin: Style.space(14)
    anchors.right: consoleBtn.left
    anchors.rightMargin: Style.space(8)
    anchors.top: parent.top
    spacing: Style.space(2)

    Text {
      id: titleText
      width: parent.width
      text: hdr.title
      textFormat: Text.PlainText
      color: hdr.foreground
      font.family: hdr.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
      elide: Text.ElideRight
    }

    Text {
      width: parent.width
      text: hdr.meta !== "" ? hdr.meta : "\u00A0"
      textFormat: Text.PlainText
      opacity: hdr.metaOpacity
      color: hdr.dim
      font.family: hdr.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }
  }

  PanelActionButton {
    id: consoleBtn
    z: 2
    anchors.right: cog.left
    anchors.rightMargin: Style.space(2)
    anchors.top: parent.top
    iconText: "󰏌"
    tooltipText: "xAI Console"
    foreground: hdr.foreground
    fontFamily: hdr.fontFamily
    onClicked: hdr.consoleClicked()
  }

  PanelActionButton {
    id: cog
    z: 2
    anchors.right: parent.right
    anchors.top: parent.top
    iconText: hdr.settingsOpen ? "󰁍" : "󰒓"
    tooltipText: hdr.settingsOpen ? "Back to usage" : "Settings"
    foreground: hdr.foreground
    fontFamily: hdr.fontFamily
    onClicked: hdr.settingsClicked()
  }
}
