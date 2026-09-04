import QtQuick

Item {
  id: mark
  property color color: "#ffffff"
  implicitWidth: 16
  implicitHeight: 16
  clip: true

  Image {
    anchors.fill: parent
    source: Qt.resolvedUrl("assets/grok.svg")
    fillMode: Image.PreserveAspectFit
    smooth: true
    sourceSize.width: Math.round(width * 2)
    sourceSize.height: Math.round(height * 2)
  }
}
