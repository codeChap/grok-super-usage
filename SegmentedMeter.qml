import QtQuick
import qs.Commons
import qs.Ui

Item {
  id: meter
  property var segments: []
  property real totalPercent: -1
  property real expectedPace: -1
  property color fillColor: Color.accent
  property color paceMarkerColor: Color.accent
  property color track: Color.background
  property color foreground: Color.foreground
  property var segmentPalette: []
  property int dayCount: 7
  property real thickness: Math.max(Style.space(6), Math.round(Style.spacing.controlHeight * 0.18))

  implicitHeight: thickness
  readonly property int dayMarkerCount: dayCount >= 2 ? dayCount - 1 : 0

  function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)) }

  function segmentColor(index) {
    var palette = meter.segmentPalette
    if (!palette || !palette.length) return meter.fillColor
    return palette[index % palette.length]
  }

  readonly property real usedFraction: {
    if (meter.totalPercent >= 0) return meter.clamp(meter.totalPercent, 0, 1)
    var sum = 0
    var segs = meter.segments || []
    for (var i = 0; i < segs.length; i++) {
      var p = Number(segs[i] && segs[i].percent)
      if (isFinite(p) && p > 0) sum += p
    }
    return meter.clamp(sum, 0, 1)
  }

  readonly property real paceFraction: {
    var p = Number(meter.expectedPace)
    if (!isFinite(p) || p < 0) return -1
    return meter.clamp(p, 0, 1)
  }

  readonly property color dayMarkerOnTrack: Qt.rgba(
    meter.foreground.r, meter.foreground.g, meter.foreground.b, 0.28)
  readonly property color dayMarkerOnFill: Qt.rgba(
    meter.track.r, meter.track.g, meter.track.b, 0.72)

  Rectangle {
    id: meterTrack
    anchors.fill: parent
    radius: height / 2
    color: meter.track
    clip: true

    Row {
      id: fillRow
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      height: parent.height
      width: parent.width * meter.usedFraction
      z: 1

      Repeater {
        model: meter.segments

        Rectangle {
          required property var modelData
          required property int index
          readonly property real pct: {
            var p = Number(modelData && modelData.percent)
            return isFinite(p) && p > 0 ? p : 0
          }
          width: {
            var used = meter.usedFraction
            if (!(used > 0) || !(pct > 0)) return 0
            return fillRow.width * (pct / used)
          }
          height: parent.height
          color: meter.segmentColor(index)
        }
      }

      Rectangle {
        visible: (!meter.segments || meter.segments.length === 0) && meter.usedFraction > 0
        width: fillRow.width
        height: parent.height
        color: meter.fillColor
      }
    }

    Item {
      anchors.fill: parent
      z: 2

      Repeater {
        model: meter.dayMarkerCount

        Rectangle {
          required property int index
          readonly property real dayFraction: (index + 1) / meter.dayCount
          readonly property bool overUsed: dayFraction <= meter.usedFraction + 0.0001
          width: Math.max(1, Math.round(Style.space(1)))
          height: Math.max(2, Math.round(parent.height * 0.78))
          radius: width / 2
          anchors.verticalCenter: parent.verticalCenter
          x: Math.round(parent.width * dayFraction - width / 2)
          color: overUsed ? meter.dayMarkerOnFill : meter.dayMarkerOnTrack
        }
      }
    }

    Rectangle {
      visible: meter.paceFraction >= 0
      z: 3
      width: Math.max(2, Math.round(Style.space(2)))
      height: parent.height
      radius: width / 2
      anchors.verticalCenter: parent.verticalCenter
      x: Math.round(parent.width * meter.paceFraction - width / 2)
      color: meter.paceMarkerColor
    }
  }
}
