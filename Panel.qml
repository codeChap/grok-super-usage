import QtQuick
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "codechap.grokbar"
  ipcTarget: "codechap.grokbar"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  readonly property var barIdentity: hostWidget || root

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color underPaceColor: Color.accent
  readonly property color overPaceColor: Color.urgent
  readonly property color paceMarkerColor: Color.accent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property color surface: Color.popups.background
  readonly property color track: Style.selectedFillFor(foreground, Color.accent)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property real rawPrimaryPercent: hostWidget ? Number(hostWidget.primaryPercent) : -1
  readonly property string resetAt: hostWidget ? String(hostWidget.resetAt || "") : ""
  readonly property string periodStart: hostWidget ? String(hostWidget.periodStart || "") : ""
  readonly property string tierLabel: hostWidget ? String(hostWidget.tierLabel || "") : ""
  property bool settingsOpen: false
  property bool refreshing: false
  property double refreshHoldUntilMs: 0
  readonly property string subscriptionPeriodEnd: hostWidget ? String(hostWidget.subscriptionPeriodEnd || "") : ""
  readonly property bool subscriptionCancelsAtEnd: hostWidget ? hostWidget.subscriptionCancelsAtEnd === true : false
  readonly property string usageStatusText: hostWidget ? String(hostWidget.usageStatusText || "") : ""
  readonly property string authHelpText: hostWidget ? String(hostWidget.authHelpText || "") : ""
  readonly property var categories: hostWidget && hostWidget.categories ? hostWidget.categories : []
  readonly property double nowMs: hostWidget ? Number(hostWidget.nowMs) : Date.now()

  readonly property bool grokHasData: rawPrimaryPercent >= 0
  readonly property bool billingHasData: hostWidget ? hostWidget.billingHasData === true : false
  readonly property string billingLabel: hostWidget ? String(hostWidget.billingLabel || "") : ""
  readonly property string billingPeriod: hostWidget ? String(hostWidget.billingPeriod || "") : ""
  readonly property string billingStatusText: hostWidget ? String(hostWidget.billingStatusText || "") : ""
  readonly property string billingHelpText: hostWidget ? String(hostWidget.billingHelpText || "") : ""
  readonly property string billingUsedLabel: billingHasData && billingLabel !== ""
    ? billingLabel + " API this cycle"
    : (billingStatusText !== "" ? billingStatusText : "")
  readonly property string managementKeyPath: {
    if (hostWidget && typeof hostWidget.setting === "function")
      return String(hostWidget.setting("managementKeyPath", "") || "")
    if (settings && settings.managementKeyPath)
      return String(settings.managementKeyPath)
    return ""
  }

  readonly property real expectedPace: {
    if (hostWidget && typeof hostWidget.expectedPace === "number"
        && isFinite(hostWidget.expectedPace) && hostWidget.expectedPace >= 0)
      return Math.max(0, Math.min(1, Number(hostWidget.expectedPace)))
    var start = root.parseTimeMs(periodStart)
    var end = root.parseTimeMs(resetAt)
    if (!(end > 0)) return -1
    if (!(start > 0) || !(start < end))
      start = end - 7 * 24 * 3600 * 1000
    var frac = (nowMs - start) / (end - start)
    if (!isFinite(frac)) return -1
    return Math.max(0, Math.min(1, frac))
  }

  readonly property real primaryPercent: rawPrimaryPercent
  readonly property bool overPace: expectedPace >= 0 && primaryPercent >= 0
    && primaryPercent > expectedPace + 0.0001
  readonly property real paceAlarmFloor: hostWidget && typeof hostWidget.paceAlarmFloor === "number"
    ? Number(hostWidget.paceAlarmFloor) : 0.15
  readonly property bool paceAlarmEnabled: hostWidget ? hostWidget.paceAlarm === true : false
  readonly property bool showWeeklyUsage: hostWidget ? hostWidget.showWeeklyUsage !== false : true
  readonly property bool showApiBilling: hostWidget ? hostWidget.showApiBilling !== false : true
  readonly property bool paceAlarming: paceAlarmEnabled && overPace && primaryPercent >= paceAlarmFloor
  readonly property color usageFillColor: paceAlarming ? overPaceColor : underPaceColor
  readonly property bool alarming: primaryPercent >= 0.9 || paceAlarming

  readonly property var productLimits: {
    var byType = {}
    var cats = root.categories
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
    for (var e = 0; e < extras.length; e++)
      out.push(extras[e])
    return out
  }

  readonly property string weeklyTitle: tierLabel !== "" ? tierLabel : "Grok"
  readonly property string grokRebillLabel: root.formatRebillLabel(subscriptionPeriodEnd, subscriptionCancelsAtEnd)
  readonly property string heroMeta: {
    if (usageStatusText !== "") return usageStatusText
    if (grokRebillLabel !== "") return grokRebillLabel
    return "\u00A0"
  }
  readonly property real grokMetaOpacity: {
    if (usageStatusText !== "") return 1
    return grokRebillLabel !== "" ? 1 : 0
  }
  readonly property string usedLabel: primaryPercent >= 0
    ? Math.round(primaryPercent * 100) + "% of weekly limit used" : ""
  readonly property string resetsLabel: root.formatResetsLabel(resetAt)

  readonly property var segmentPalette: {
    var base = root.usageFillColor
    return [
      base,
      Qt.rgba(base.r, base.g, base.b, 0.72),
      Qt.rgba(base.r, base.g, base.b, 0.50),
      Qt.rgba(base.r, base.g, base.b, 0.86),
      Qt.rgba(base.r, base.g, base.b, 0.60)
    ]
  }

  readonly property url iconSource: colorLuminance(surface) >= 0.5
    ? Qt.resolvedUrl("assets/grok-light.svg")
    : Qt.resolvedUrl("assets/grok.svg")

  function parseTimeMs(value) {
    var text = String(value || "").trim()
    if (text === "") return NaN
    var t = new Date(text).getTime()
    return isFinite(t) ? t : NaN
  }

  function colorChannelLuminance(value) {
    var channel = Number(value)
    if (!isFinite(channel)) return 0
    return channel <= 0.03928 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4)
  }

  function colorLuminance(color) {
    return 0.2126 * colorChannelLuminance(color.r)
      + 0.7152 * colorChannelLuminance(color.g)
      + 0.0722 * colorChannelLuminance(color.b)
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
    var when = root.parseResetWhen(iso)
    if (!when) return ""
    var h = when.getHours()
    var min = when.getMinutes()
    var ampm = h >= 12 ? "PM" : "AM"
    var h12 = h % 12
    if (h12 === 0) h12 = 12
    var timePart = min > 0
      ? (h12 + ":" + (min < 10 ? "0" : "") + min + ampm)
      : (h12 + ampm)
    return "Resets " + root.shortMonthName(when) + " " + when.getDate() + ", " + timePart
  }

  function formatRebillLabel(iso, cancels) {
    var when = root.parseResetWhen(iso)
    if (!when) return ""
    var verb = cancels === true ? "expires" : "renews"
    return verb + " " + root.shortMonthName(when) + " " + when.getDate()
      + ", " + when.getFullYear()
  }

  function setCenterHoverRevealSuppressed(value) {
    if (root.bar && "centerHoverRevealSuppressed" in root.bar)
      root.bar.centerHoverRevealSuppressed = value
  }

  function syncRefreshing() {
    var live = false
    if (hostWidget) {
      live = hostWidget.refreshing === true || hostWidget.billingRefreshing === true
    }
    if (!live && Date.now() < root.refreshHoldUntilMs)
      live = true
    root.refreshing = live
  }

  function open() {
    root.controller.show()
    root.refresh()
    Qt.callLater(function() {
      if (root.opened) setCenterHoverRevealSuppressed(true)
    })
  }

  function openFromHotkey() { open() }

  function close() {
    root.settingsOpen = false
    setCenterHoverRevealSuppressed(false)
    root.controller.hide()
  }

  function openSettings() { root.settingsOpen = true }
  function closeSettings() { root.settingsOpen = false }

  function saveManagementKeyPath(path) {
    var next = String(path || "").trim()
    if (hostWidget && typeof hostWidget.persistSettings === "function")
      hostWidget.persistSettings({ managementKeyPath: next })
    if (hostWidget && typeof hostWidget.probeBilling === "function")
      hostWidget.probeBilling()
  }

  function setFlag(key, on) {
    var values = {}
    values[key] = on === true
    if (hostWidget && typeof hostWidget.persistSettings === "function")
      hostWidget.persistSettings(values)
    if (key === "showApiBilling" && on && hostWidget) {
      if (typeof hostWidget.probeBilling === "function")
        hostWidget.probeBilling()
      if (typeof hostWidget.refreshBilling === "function")
        hostWidget.refreshBilling()
    }
  }

  function toggle() {
    if (root.opened) close()
    else open()
  }

  function refresh() {
    if (!hostWidget || typeof hostWidget.refresh !== "function")
      return
    root.refreshHoldUntilMs = Date.now() + 480
    root.refreshing = true
    hostWidget.refresh()
    Qt.callLater(root.syncRefreshing)
  }

  function switchPanel(direction) {
    if (root.bar && typeof root.bar.switchPanelFrom === "function")
      return root.bar.switchPanelFrom(root.barIdentity, direction)
    return false
  }

  onHostWidgetChanged: root.syncRefreshing()

  Connections {
    target: root.hostWidget
    enabled: root.hostWidget != null
    function onRefreshingChanged() { root.syncRefreshing() }
    function onBillingRefreshingChanged() { root.syncRefreshing() }
  }

  Timer {
    interval: 80
    running: root.refreshing && root.refreshHoldUntilMs > 0 && root.opened
    repeat: true
    onTriggered: {
      if (Date.now() >= root.refreshHoldUntilMs)
        root.syncRefreshing()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(400))
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(520))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: root.settingsOpen
      onActivateRequested: if (!root.settingsOpen) root.refresh()
      onCloseRequested: root.settingsOpen ? root.closeSettings() : root.close()
      onTabRequested: function(direction) {
        if (!root.settingsOpen) root.switchPanel(direction)
      }
      onTextKey: function(t) {
        if (root.settingsOpen) return
        if (t === "r" || t === "R") root.refresh()
        if (t === "s" || t === "S") root.openSettings()
      }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        Column {
          visible: root.settingsOpen || root.grokHasData || root.usageStatusText !== ""
          width: parent.width
          spacing: Style.space(12)

          PlanHeader {
            width: parent.width
            title: root.settingsOpen ? "Settings" : root.weeklyTitle
            meta: root.settingsOpen ? "api billing" : root.heroMeta
            metaOpacity: root.settingsOpen ? 1 : root.grokMetaOpacity
            iconSource: root.iconSource
            settingsOpen: root.settingsOpen
            foreground: root.foreground
            dim: root.dim
            fontFamily: root.fontFamily
            onSettingsClicked: root.settingsOpen = !root.settingsOpen
          }

          BorderSurface {
            visible: root.usageStatusText !== ""
            width: parent.width
            implicitHeight: statusText.implicitHeight + Style.spacing.xl * 2
            color: Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.10)
            borderSpec: Border.flat(Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.35), 1)
            radius: Style.cornerRadius

            Text {
              id: statusText
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              anchors.leftMargin: Style.space(12)
              anchors.rightMargin: Style.space(12)
              text: root.authHelpText !== "" ? root.authHelpText : root.usageStatusText
              textFormat: Text.PlainText
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }
          }

          PanelSeparator {
            visible: usageSection.visible
            foreground: root.foreground
          }

          SettingsForm {
            visible: root.settingsOpen
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
            onFlagChanged: function(key, on) { root.setFlag(key, on) }
            onKeyPathCommitted: function(path) { root.saveManagementKeyPath(path) }
          }

          Column {
            id: usageSection
            visible: !root.settingsOpen && root.primaryPercent >= 0
            width: parent.width
            spacing: Style.space(10)

            Item {
              width: parent.width
              implicitHeight: Math.max(usedText.implicitHeight, resetsText.implicitHeight)

              Text {
                id: usedText
                text: root.usedLabel
                color: root.alarming ? root.urgent : root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
              }

              Text {
                id: resetsText
                visible: text !== ""
                text: root.resetsLabel
                color: root.dim
                font.family: root.fontFamily
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
              visible: root.productLimits.length > 0 || root.primaryPercent >= 0
              segments: root.productLimits
              totalPercent: root.primaryPercent
              expectedPace: root.expectedPace
              fillColor: root.usageFillColor
              paceMarkerColor: root.paceMarkerColor
              track: root.track
              foreground: root.foreground
              segmentPalette: root.segmentPalette
            }

            Flow {
              visible: root.primaryPercent >= 0
              width: parent.width
              spacing: Style.space(12)

              Repeater {
                model: ["Grok Build", "Chat", "Imagine"]

                Row {
                  required property string modelData
                  required property int index
                  readonly property real pct: {
                    var item = root.productLimits[index]
                    var p = Number(item && item.percent)
                    if (!isFinite(p) || p < 0) return 0
                    return p
                  }
                  spacing: Style.space(5)

                  Rectangle {
                    width: Style.space(6)
                    height: Style.space(6)
                    radius: width / 2
                    anchors.verticalCenter: parent.verticalCenter
                    color: {
                      var pal = root.segmentPalette
                      return pal && pal.length ? pal[index % pal.length] : root.usageFillColor
                    }
                  }

                  Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData + " " + Math.round(pct * 100) + "%"
                    color: root.dim
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.caption
                    renderType: Text.NativeRendering
                  }
                }
              }
            }
          }

          Column {
            visible: !root.settingsOpen
              && (root.billingUsedLabel !== "" || root.billingHelpText !== "")
            width: parent.width
            spacing: Style.space(10)

            PanelSeparator {
              visible: usageSection.visible
              foreground: root.foreground
            }

            Item {
              visible: root.billingUsedLabel !== ""
              width: parent.width
              implicitHeight: Math.max(billUsedText.implicitHeight, billPeriodText.implicitHeight)

              Text {
                id: billUsedText
                text: root.billingHasData
                  ? root.billingLabel + " of API bill this cycle"
                  : root.billingUsedLabel
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                  - (billPeriodText.visible ? billPeriodText.implicitWidth + Style.space(10) : 0)
                wrapMode: Text.WordWrap
              }

              Text {
                id: billPeriodText
                visible: root.billingPeriod !== ""
                text: root.billingPeriod
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
              }
            }

            Text {
              visible: !root.billingHasData && root.billingHelpText !== ""
              width: parent.width
              text: root.billingHelpText
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }
          }
        }
      }
    }
  }
}
