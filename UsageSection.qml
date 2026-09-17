pragma ComponentBehavior: Bound
import QtQuick
import qs.Commons
import qs.Ui

Column {
  id: section
  property var usage: ({})
  property bool showAccountLabel: false
  property double nowMs: Date.now()
  property bool paceAlarmEnabled: false
  property real paceAlarmFloor: 0.15
  property color foreground: Color.foreground
  property color urgent: Color.urgent
  property color dim: Color.foreground
  property color underPaceColor: Color.accent
  property color overPaceColor: Color.urgent
  property color paceMarkerColor: Color.accent
  property color track: Color.background
  property string fontFamily: Style.font.family

  width: parent ? parent.width : implicitWidth
  spacing: Style.space(10)

  readonly property real primaryPercent: {
    var n = Number(section.usage && section.usage.rateLimitPercent)
    return isFinite(n) ? n : -1
  }
  readonly property string resetAt: section.usage ? String(section.usage.rateLimitResetAt || "") : ""
  readonly property string periodStart: section.usage ? String(section.usage.rateLimitPeriodStart || "") : ""
  readonly property var categories: section.usage && section.usage.categories ? section.usage.categories : []
  readonly property int prepaidCredits: section.usage ? Number(section.usage.prepaidCredits) || 0 : 0
  readonly property string usageStatusText: section.usage ? String(section.usage.usageStatusText || "") : ""
  readonly property string authHelpText: section.usage ? String(section.usage.authHelpText || "") : ""
  readonly property string accountEmail: section.usage ? String(section.usage.accountEmail || "") : ""
  readonly property string accountName: section.usage ? String(section.usage.accountName || "") : ""
  readonly property bool saved: section.usage ? section.usage.saved === true : false
  readonly property bool onCredits: primaryPercent >= 1.0 && prepaidCredits > 0
  readonly property bool hasMeter: primaryPercent >= 0
  readonly property bool hasStatus: usageStatusText !== ""

  readonly property string accountLabel: {
    var who = accountEmail !== "" ? accountEmail
      : (accountName !== "" ? accountName : (saved ? "Saved login" : "This login"))
    if (!saved && section.showAccountLabel) return "Current · " + who
    return who
  }

  readonly property var productLimits: {
    var byType = {}
    var cats = section.categories
    if (cats && cats.length) {
      for (var i = 0; i < cats.length; i++) {
        var c = cats[i]
        if (!c) continue
        var t = Number(c.type)
        if (!isFinite(t)) continue
        var pct = Number(c.percent)
        if (!isFinite(pct) || pct < 0) pct = 0
        byType[t] = { title: String(c.title || "Product"), type: t, percent: pct }
      }
    }
    var core = [
      { type: 2, title: "Grok Build" },
      { type: 4, title: "Chat" },
      { type: 5, title: "Imagine" }
    ]
    var out = []
    for (var k = 0; k < core.length; k++) {
      var want = core[k]
      out.push(byType[want.type] || { title: want.title, type: want.type, percent: 0 })
    }
    var extras = []
    for (var key in byType) {
      var item = byType[key]
      if (item.type === 2 || item.type === 4 || item.type === 5) continue
      if (!(item.percent > 0)) continue
      extras.push(item)
    }
    extras.sort(function(a, b) { return a.type - b.type })
    var maxOut = 16
    for (var e = 0; e < extras.length && out.length < maxOut; e++)
      out.push(extras[e])
    return out
  }

  readonly property real expectedPace: {
    var start = section.parseTimeMs(periodStart)
    var end = section.parseTimeMs(resetAt)
    if (!(end > 0)) return -1
    if (!(start > 0) || !(start < end))
      start = end - 7 * 24 * 3600 * 1000
    var frac = (nowMs - start) / (end - start)
    if (!isFinite(frac)) return -1
    return Math.max(0, Math.min(1, frac))
  }

  readonly property bool overPace: expectedPace >= 0 && primaryPercent >= 0
    && primaryPercent > expectedPace + 0.0001
  readonly property bool paceAlarming: paceAlarmEnabled && overPace && primaryPercent >= paceAlarmFloor
  readonly property bool alarming: primaryPercent >= 0.9 || paceAlarming
  readonly property color usageFillColor: paceAlarming ? overPaceColor : underPaceColor
  readonly property var segmentPalette: {
    var base = section.usageFillColor
    return [
      base,
      Qt.rgba(base.r, base.g, base.b, 0.72),
      Qt.rgba(base.r, base.g, base.b, 0.50),
      Qt.rgba(base.r, base.g, base.b, 0.86),
      Qt.rgba(base.r, base.g, base.b, 0.60)
    ]
  }

  readonly property string usedLabel: {
    if (primaryPercent < 0) return ""
    var pct = Math.round(primaryPercent * 100) + "% of weekly limit used"
    if (onCredits) pct += " ($" + (prepaidCredits / 100).toFixed(2) + ")"
    return pct
  }
  readonly property string resetsLabel: section.formatResetsLabel(resetAt)

  function parseTimeMs(value) {
    var text = String(value || "").trim()
    if (text === "") return NaN
    var t = new Date(text).getTime()
    return isFinite(t) ? t : NaN
  }

  function parseResetWhen(iso) {
    var text = String(iso || "").trim()
    if (text === "") return null
    var when = new Date(text)
    var t = when.getTime()
    if (isFinite(t)) return when
    var m = text.match(/^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2}):(\d{2})/)
    if (!m) return null
    t = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]),
                 Number(m[4]), Number(m[5]), Number(m[6]))
    when = new Date(t)
    return isFinite(t) ? when : null
  }

  function shortMonthName(when) {
    var months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    return months[when.getMonth()]
  }

  function formatResetsLabel(iso) {
    var when = section.parseResetWhen(iso)
    if (!when) return ""
    var h = when.getHours()
    var min = when.getMinutes()
    var ampm = h >= 12 ? "PM" : "AM"
    var h12 = h % 12
    if (h12 === 0) h12 = 12
    var timePart = min > 0
      ? (h12 + ":" + (min < 10 ? "0" : "") + min + ampm)
      : (h12 + ampm)
    return "Resets " + section.shortMonthName(when) + " " + when.getDate() + ", " + timePart
  }

  visible: showAccountLabel || hasMeter || hasStatus

  Text {
    visible: section.showAccountLabel
    width: parent.width
    text: section.accountLabel
    textFormat: Text.PlainText
    color: section.dim
    font.family: section.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  BorderSurface {
    visible: section.hasStatus
    width: parent.width
    implicitHeight: statusText.implicitHeight + Style.spacing.xl * 2
    color: Qt.rgba(section.urgent.r, section.urgent.g, section.urgent.b, 0.10)
    borderSpec: Border.flat(Qt.rgba(section.urgent.r, section.urgent.g, section.urgent.b, 0.35), 1)
    radius: Style.cornerRadius

    Text {
      id: statusText
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(12)
      anchors.rightMargin: Style.space(12)
      text: section.authHelpText !== "" ? section.authHelpText : section.usageStatusText
      textFormat: Text.PlainText
      color: section.dim
      font.family: section.fontFamily
      font.pixelSize: Style.font.caption
      wrapMode: Text.WordWrap
    }
  }

  Item {
    visible: section.hasMeter
    width: parent.width
    implicitHeight: Math.max(usedText.implicitHeight, resetsText.implicitHeight)

    Text {
      id: usedText
      text: section.usedLabel
      textFormat: Text.PlainText
      color: section.alarming ? section.urgent : section.foreground
      font.family: section.fontFamily
      font.pixelSize: Style.font.body
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
    }

    Text {
      id: resetsText
      visible: text !== ""
      text: section.resetsLabel
      textFormat: Text.PlainText
      color: section.dim
      font.family: section.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideLeft
      horizontalAlignment: Text.AlignRight
      anchors.right: parent.right
      anchors.left: usedText.right
      anchors.leftMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
    }
  }

  SegmentedMeter {
    width: parent.width
    visible: section.hasMeter
    segments: section.productLimits
    totalPercent: section.primaryPercent
    expectedPace: section.expectedPace
    fillColor: section.usageFillColor
    paceMarkerColor: section.paceMarkerColor
    track: section.track
    foreground: section.foreground
    segmentPalette: section.segmentPalette
  }

  Flow {
    visible: section.hasMeter
    width: parent.width
    spacing: Style.space(12)

    Repeater {
      model: [
        { title: "Grok Build", type: 2 },
        { title: "Chat", type: 4 },
        { title: "Imagine", type: 5 }
      ]

      Row {
        required property var modelData
        required property int index
        readonly property real pct: {
          var want = Number(modelData && modelData.type)
          var segs = section.productLimits || []
          for (var i = 0; i < segs.length; i++) {
            if (Number(segs[i] && segs[i].type) !== want) continue
            var p = Number(segs[i].percent)
            if (!isFinite(p) || p < 0) return 0
            return p
          }
          return 0
        }
        spacing: Style.space(5)

        Rectangle {
          width: Style.space(6)
          height: Style.space(6)
          radius: width / 2
          anchors.verticalCenter: parent.verticalCenter
          color: {
            var pal = section.segmentPalette
            return pal && pal.length ? pal[index % pal.length] : section.usageFillColor
          }
        }

        Text {
          anchors.verticalCenter: parent.verticalCenter
          text: String(modelData.title || "") + " " + Math.round(pct * 100) + "%"
          textFormat: Text.PlainText
          color: section.dim
          font.family: section.fontFamily
          font.pixelSize: Style.font.caption
          renderType: Text.NativeRendering
        }
      }
    }
  }
}
