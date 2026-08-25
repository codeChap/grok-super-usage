import QtQuick
import qs.Commons
import qs.Ui

// Usage popup. BarWidget.qml owns the bar slot and scan state.
// Grok card mirrors grok.com Settings → Usage (weekly pool + products).
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
  // Theme aliases only — no hardcoded greens/reds.
  // Under pace: accent; over pace: urgent.
  // Pace marker uses full accent so it reads apart from faint day ticks.
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
  property string grokLoginName: ""
  property string grokLoginEmail: ""
  property bool grokIdentityOpen: false
  property bool settingsOpen: false
  property bool cursorIdentityOpen: false
  property bool refreshing: false
  property int refreshDotFrame: 0
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
  readonly property bool showCursorUsage: hostWidget
    ? hostWidget.showCursorUsage === true
    : !!(settings && settings.showCursorUsage === true)
  readonly property real cursorAutoPercent: hostWidget ? Number(hostWidget.cursorAutoPercent) : -1
  readonly property real cursorApiPercent: hostWidget ? Number(hostWidget.cursorApiPercent) : -1
  readonly property string cursorResetAt: hostWidget ? String(hostWidget.cursorResetAt || "") : ""
  readonly property string cursorPeriodStart: hostWidget ? String(hostWidget.cursorPeriodStart || "") : ""
  readonly property string cursorTierLabel: hostWidget ? String(hostWidget.cursorTierLabel || "") : ""
  property string cursorLoginName: ""
  property string cursorLoginEmail: ""
  readonly property string cursorUsageStatusText: hostWidget ? String(hostWidget.cursorUsageStatusText || "") : ""
  readonly property string cursorAuthHelpText: hostWidget ? String(hostWidget.cursorAuthHelpText || "") : ""
  readonly property bool cursorHasData: cursorAutoPercent >= 0 || cursorApiPercent >= 0

  // TEMP QA hook: force over-pace styling (leave false in production).
  readonly property bool simulateOverPace: false

  // Linear expected usage by now: elapsed / period length (0–1).
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

  // Displayed usage: when simulating, push past the pace marker (~+15pp, min past pace).
  readonly property real primaryPercent: {
    var raw = rawPrimaryPercent
    if (!root.simulateOverPace || !(raw >= 0) || !(expectedPace >= 0))
      return raw
    var bumped = Math.max(raw, expectedPace + 0.15)
    return Math.max(0, Math.min(1, bumped))
  }

  readonly property bool overPace: expectedPace >= 0 && primaryPercent >= 0
    && primaryPercent > expectedPace + 0.0001
  readonly property real paceAlarmFloor: hostWidget && typeof hostWidget.paceAlarmFloor === "number"
    ? Number(hostWidget.paceAlarmFloor)
    : 0.15
  readonly property bool paceAlarmEnabled: hostWidget ? hostWidget.paceAlarm === true : false
  readonly property bool showWeeklyUsage: hostWidget ? hostWidget.showWeeklyUsage !== false : true
  readonly property bool showApiBilling: hostWidget ? hostWidget.showApiBilling !== false : true
  readonly property bool paceAlarming: paceAlarmEnabled && overPace && primaryPercent >= paceAlarmFloor
  readonly property color usageFillColor: paceAlarming ? overPaceColor : underPaceColor

  // Same product split as the old i3 bar (B / C / I), including 0%.
  // Extra products (API, Voice, …) only appear when they have used some.
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
        byType[t] = {
          title: String(c.title || "Product"),
          type: t,
          percent: pct
        }
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
      if (byType[want.type])
        out.push(byType[want.type])
      else
        out.push({ title: want.title, type: want.type, percent: 0 })
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

  // Hero title: subscription type only, e.g. "SuperGrok Heavy"
  readonly property string weeklyTitle: {
    if (tierLabel !== "") return tierLabel
    return "Grok"
  }

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

  // Same four frames as Grok Build's Working spinner (one cell, not ASCII :).
  readonly property bool panelRefreshing: {
    if (root.refreshing) return true
    if (!hostWidget) return false
    if (hostWidget.refreshing === true) return true
    return hostWidget.billingRefreshing === true
  }
  readonly property string refreshDotsText: {
    var frames = ["\u22C5", "\u2236", "\u2E2C", "\u2059"] // ⋅ ∶ ⸬ ⁙
    return frames[root.refreshDotFrame % 4]
  }

  // "23% of weekly limit used"
  readonly property string usedLabel: primaryPercent >= 0
    ? Math.round(primaryPercent * 100) + "% of weekly limit used"
    : ""

  // "Resets Aug 13, 9AM" (short month, no year)
  readonly property string resetsLabel: root.formatResetsLabel(resetAt)

  readonly property bool alarming: primaryPercent >= 0.9 || paceAlarming

  readonly property real cursorExpectedPace: {
    if (hostWidget && typeof hostWidget.cursorExpectedPace === "number"
        && isFinite(hostWidget.cursorExpectedPace) && hostWidget.cursorExpectedPace >= 0)
      return Math.max(0, Math.min(1, Number(hostWidget.cursorExpectedPace)))
    var start = root.parseTimeMs(cursorPeriodStart)
    var end = root.parseTimeMs(cursorResetAt)
    if (!(end > 0)) return -1
    if (!(start > 0) || !(start < end))
      start = end - 30 * 24 * 3600 * 1000
    var frac = (nowMs - start) / (end - start)
    if (!isFinite(frac)) return -1
    return Math.max(0, Math.min(1, frac))
  }

  readonly property real cursorAutoDisplay: {
    var raw = cursorAutoPercent
    if (!root.simulateOverPace || !(raw >= 0) || !(cursorExpectedPace >= 0))
      return raw
    return Math.max(0, Math.min(1, Math.max(raw, cursorExpectedPace + 0.15)))
  }
  readonly property real cursorApiDisplay: {
    var raw = cursorApiPercent
    if (!root.simulateOverPace || !(raw >= 0) || !(cursorExpectedPace >= 0))
      return raw
    return Math.max(0, Math.min(1, Math.max(raw, cursorExpectedPace + 0.15)))
  }
  readonly property bool cursorAutoOverPace: cursorExpectedPace >= 0 && cursorAutoDisplay >= 0
    && cursorAutoDisplay > cursorExpectedPace + 0.0001
  readonly property bool cursorApiOverPace: cursorExpectedPace >= 0 && cursorApiDisplay >= 0
    && cursorApiDisplay > cursorExpectedPace + 0.0001

  readonly property var cursorPools: {
    var out = []
    if (cursorAutoDisplay >= 0)
      out.push({ title: "Cursor Models", percent: cursorAutoDisplay, overPace: cursorAutoOverPace })
    if (cursorApiDisplay >= 0)
      out.push({ title: "Other Models", percent: cursorApiDisplay, overPace: cursorApiOverPace })
    return out
  }

  readonly property string cursorTitle: cursorTierLabel !== "" ? cursorTierLabel : "Cursor"
  readonly property string cursorRebillLabel: root.formatRebillLabel(cursorResetAt, false)
  readonly property string cursorHeroMeta: {
    if (cursorUsageStatusText !== "") return cursorUsageStatusText
    if (cursorRebillLabel !== "") return cursorRebillLabel
    return "\u00A0"
  }
  readonly property real cursorMetaOpacity: {
    if (cursorUsageStatusText !== "") return 1
    return root.cursorIdentityOpen && cursorRebillLabel !== "" ? 1 : 0
  }
  readonly property string cursorResetsLabel: root.formatResetsLabel(cursorResetAt)
  readonly property url cursorIconSource: colorLuminance(surface) >= 0.5
    ? Qt.resolvedUrl("assets/cursor-light.svg")
    : Qt.resolvedUrl("assets/cursor.svg")

  // Segment shades of the pace-aware fill color (accent under, urgent over).
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

  function categoryTypeId(c) {
    if (!c) return NaN
    var t = Number(c.typeId)
    if (isFinite(t)) return t
    t = Number(c.type)
    return t
  }

  function percentForType(typeId) {
    var cats = root.categories
    if (!cats || !cats.length) return 0
    for (var i = 0; i < cats.length; i++) {
      if (root.categoryTypeId(cats[i]) !== typeId) continue
      var pct = Number(cats[i].percent)
      if (!isFinite(pct) || pct < 0) return 0
      return pct
    }
    return 0
  }

  function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)) }

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

  // "Resets Aug 13, 9AM" (local time; minutes only when not :00).
  function formatResetsLabel(iso) {
    var when = root.parseResetWhen(iso)
    if (!when) return ""
    var months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    var h = when.getHours()
    var min = when.getMinutes()
    var ampm = h >= 12 ? "PM" : "AM"
    var h12 = h % 12
    if (h12 === 0) h12 = 12
    var timePart = min > 0
      ? (h12 + ":" + (min < 10 ? "0" : "") + min + ampm)
      : (h12 + ampm)
    return "Resets " + months[when.getMonth()] + " " + when.getDate()
      + ", " + timePart
  }

  // Subscription rebill/expiry under the plan title (PanelHero.meta is uppercase).
  function formatRebillLabel(iso, cancels) {
    var when = root.parseResetWhen(iso)
    if (!when) return ""
    var months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                  "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
    var verb = cancels === true ? "expires" : "renews"
    return verb + " " + months[when.getMonth()] + " " + when.getDate()
      + ", " + when.getFullYear()
  }

  function segmentColor(index) {
    var palette = root.segmentPalette
    if (!palette || !palette.length) return root.usageFillColor
    return palette[index % palette.length]
  }

  function setCenterHoverRevealSuppressed(value) {
    if (root.bar && "centerHoverRevealSuppressed" in root.bar)
      root.bar.centerHoverRevealSuppressed = value
  }

  function syncRefreshing() {
    var live = false
    if (hostWidget) {
      live = hostWidget.refreshing === true
        || hostWidget.billingRefreshing === true
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
    root.grokIdentityOpen = false
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
    if (key === "showApiBilling" && on && hostWidget && typeof hostWidget.probeBilling === "function")
      hostWidget.probeBilling()
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
  onRefreshingChanged: if (refreshing) refreshDotFrame = 0

  Connections {
    target: root.hostWidget
    enabled: root.hostWidget != null
    function onRefreshingChanged() { root.syncRefreshing() }
    function onBillingRefreshingChanged() { root.syncRefreshing() }
  }

  Timer {
    interval: 160
    running: root.panelRefreshing && root.opened
    repeat: true
    onTriggered: root.refreshDotFrame = (root.refreshDotFrame + 1) % 4
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

        // Grok card. Hidden when there is nothing to say about Grok.
        Column {
          id: grokCard
          visible: root.settingsOpen || root.grokHasData || root.usageStatusText !== ""
          width: parent.width
          spacing: Style.space(12)

        PlanHeader {
          id: grokHeader
          width: parent.width
          title: root.settingsOpen ? "Settings" : root.weeklyTitle
          meta: root.settingsOpen ? "api billing" : root.heroMeta
          metaOpacity: root.settingsOpen ? 1 : root.grokMetaOpacity
          iconSource: root.iconSource
          accountName: ""
          accountEmail: ""
          identityVisible: false
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

        // Usage body (no section header — title lives in the hero).
        Column {
          id: settingsSection
          visible: root.settingsOpen
          width: parent.width
          spacing: Style.space(10)

          Toggle {
            width: parent.width
            label: "Show weekly usage"
            description: "Show weekly percent and reset on the bar. The panel always has the full breakdown."
            checked: root.showWeeklyUsage
            foreground: root.foreground
            fontFamily: root.fontFamily
            onClicked: root.setFlag("showWeeklyUsage", !root.showWeeklyUsage)
          }

          Toggle {
            width: parent.width
            label: "Show API billing"
            description: "Show the API invoice amount on the bar. The panel always has the bill."
            checked: root.showApiBilling
            foreground: root.foreground
            fontFamily: root.fontFamily
            onClicked: root.setFlag("showApiBilling", !root.showApiBilling)
          }

          Toggle {
            width: parent.width
            label: "Pace warning"
            description: "Turn the bar red when weekly usage is ahead of an even burn through the week. Off by default."
            checked: root.paceAlarmEnabled
            foreground: root.foreground
            fontFamily: root.fontFamily
            onClicked: root.setFlag("paceAlarm", !root.paceAlarmEnabled)
          }

          Text {
            width: parent.width
            text: "Management key file"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }

          Text {
            width: parent.width
            text: "Key or path to file with key"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }

          TextField {
            id: keyPathField
            width: parent.width
            text: root.managementKeyPath
            placeholderText: "~/dev/XAI-MGMT-KEY.txt or xai-…"
            foreground: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            onEditingFinished: root.saveManagementKeyPath(text)
          }

          Text {
            width: parent.width
            visible: root.billingHasData
            text: "Current API bill: " + root.billingLabel
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            width: parent.width
            visible: !root.billingHasData && root.billingHelpText !== ""
            text: root.billingHelpText
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            wrapMode: Text.WordWrap
          }
        }

        Column {
          id: usageSection
          visible: !root.settingsOpen && root.primaryPercent >= 0
          width: parent.width
          spacing: Style.space(10)

          // "23% used" ……………… "Resets August 13, 2026 at 9:46 PM"
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

          // Segmented weekly bar (Chat | Grok Build | …) + day ticks + pace marker.
          SegmentedMeter {
            width: parent.width
            visible: root.productLimits.length > 0 || root.primaryPercent >= 0
            segments: root.productLimits
            totalPercent: root.primaryPercent
            expectedPace: root.expectedPace
            overPace: root.overPace
            fillColor: root.usageFillColor
            paceMarkerColor: root.paceMarkerColor
          }

          // Always Build / Chat / Imagine, including 0% — same split as i3-grok-usage.
          Flow {
            id: legend
            visible: root.primaryPercent >= 0
            width: parent.width
            spacing: Style.space(12)

            Repeater {
              model: ["Grok Build", "Chat", "Imagine"]

              Row {
                required property string modelData
                required property int index
                readonly property var typeIds: [2, 4, 5]
                readonly property real pct: root.percentForType(typeIds[index])
                spacing: Style.space(5)

                Rectangle {
                  width: Style.space(6)
                  height: Style.space(6)
                  radius: width / 2
                  anchors.verticalCenter: parent.verticalCenter
                  color: root.segmentColor(index)
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

  // Icon is centered on the title row so the plan name lines up with the logo.
  // Renewal, name, and email keep their slots and only change opacity.
  // Plan name, meta, and account identity are API strings: PlainText so
  // QML AutoText cannot treat crafted <img src> markup as a resource fetch.
  component PlanHeader: Item {
    id: hdr
    property string title: ""
    property string meta: ""
    property real metaOpacity: 0
    property url iconSource
    property string accountName: ""
    property string accountEmail: ""
    property bool identityVisible: false
    property bool settingsOpen: false
    property color foreground: Color.foreground
    property color dim: Color.foreground
    property string fontFamily: Style.font.family
    signal settingsClicked()

    implicitHeight: Math.max(iconBox.height, titleCol.implicitHeight, trail.implicitHeight)

    Item {
      id: iconBox
      width: Style.font.display
      height: titleText.height
      anchors.left: parent.left
      anchors.top: parent.top

      Image {
        anchors.centerIn: parent
        source: hdr.iconSource
        width: Style.font.display
        height: Style.font.display
        sourceSize.width: Style.font.display * 2
        sourceSize.height: Style.font.display * 2
        fillMode: Image.PreserveAspectFit
      }
    }

    Column {
      id: titleCol
      anchors.left: iconBox.right
      anchors.leftMargin: Style.space(14)
      anchors.right: trail.left
      anchors.rightMargin: Style.space(12)
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
        font.bold: false
        elide: Text.ElideRight
      }
    }

    AccountTrail {
      id: trail
      z: 2
      anchors.right: parent.right
      anchors.top: parent.top
      accountName: hdr.accountName
      accountEmail: hdr.accountEmail
      identityVisible: hdr.identityVisible
      settingsOpen: hdr.settingsOpen
      foreground: hdr.foreground
      dim: hdr.dim
      fontFamily: hdr.fontFamily
      onSettingsClicked: hdr.settingsClicked()
    }
  }

  // Name sits on the title row; email on the reserved subtitle row.
  // Cog opens the management-key settings form.
  component AccountTrail: Row {
    id: trail
    property string accountName: ""
    property string accountEmail: ""
    property bool identityVisible: false
    property bool settingsOpen: false
    property color foreground: Color.foreground
    property color dim: Color.foreground
    property string fontFamily: Style.font.family
    spacing: Style.space(10)
    signal settingsClicked()

    readonly property string titleText: accountName !== "" ? accountName : accountEmail
    readonly property string subtitleText: accountName !== "" ? accountEmail : ""

    Column {
      spacing: Style.space(2)
      opacity: trail.identityVisible && trail.titleText !== "" ? 1 : 0

      Text {
        id: nameText
        text: trail.titleText !== "" ? trail.titleText : " "
        textFormat: Text.PlainText
        color: trail.foreground
        font.family: trail.fontFamily
        font.pixelSize: Style.font.title
        font.bold: true
        horizontalAlignment: Text.AlignRight
        width: Math.max(implicitWidth, subText.implicitWidth)
        elide: Text.ElideRight
      }

      Text {
        id: subText
        text: trail.subtitleText !== "" ? trail.subtitleText : " "
        textFormat: Text.PlainText
        color: trail.dim
        font.family: trail.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        font.letterSpacing: 1.2
        horizontalAlignment: Text.AlignRight
        width: parent.width
        elide: Text.ElideRight
      }
    }

    PanelActionButton {
      anchors.verticalCenter: nameText.verticalCenter
      iconText: trail.settingsOpen ? "󰁍" : "󰒓"
      tooltipText: trail.settingsOpen ? "Back to usage" : "Settings"
      foreground: trail.foreground
      fontFamily: trail.fontFamily
      onClicked: trail.settingsClicked()
    }
  }

  // Full-width track with product slices left-to-right (pool fractions).
  // Day ticks + expected-pace marker (elapsed / period).
  // Fill uses Color.accent under pace, Color.urgent over pace.
  component SegmentedMeter: Item {
    id: meter
    property var segments: []
    property real totalPercent: -1
    property real expectedPace: -1
    property bool overPace: false
    property color fillColor: root.usageFillColor
    property color paceMarkerColor: root.paceMarkerColor
    // SuperGrok weekly pool = 7 calendar days.
    property int dayCount: 7
    property real thickness: Math.max(Style.space(6), Math.round(Style.spacing.controlHeight * 0.18))

    implicitHeight: thickness

    // dayCount < 2 → no ticks (monthly Cursor pools use dayCount: 0).
    readonly property int dayMarkerCount: dayCount >= 2 ? dayCount - 1 : 0

    readonly property real usedFraction: {
      if (meter.totalPercent >= 0) return root.clamp(meter.totalPercent, 0, 1)
      var sum = 0
      var segs = meter.segments || []
      for (var i = 0; i < segs.length; i++) {
        var p = Number(segs[i] && segs[i].percent)
        if (isFinite(p) && p > 0) sum += p
      }
      return root.clamp(sum, 0, 1)
    }

    readonly property real paceFraction: {
      var p = Number(meter.expectedPace)
      if (!isFinite(p) || p < 0) return -1
      return root.clamp(p, 0, 1)
    }

    // Day ticks: fainter on empty track, inverted/higher-contrast over used fill.
    readonly property color dayMarkerOnTrack: Qt.rgba(
      root.foreground.r, root.foreground.g, root.foreground.b, 0.28)
    readonly property color dayMarkerOnFill: Qt.rgba(
      root.track.r, root.track.g, root.track.b, 0.72)

    Rectangle {
      id: meterTrack
      anchors.fill: parent
      radius: height / 2
      color: root.track
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
            color: root.segmentColor(index)
          }
        }

        // Solid fill when we have total % but no product slices yet.
        Rectangle {
          visible: (!meter.segments || meter.segments.length === 0) && meter.usedFraction > 0
          width: fillRow.width
          height: parent.height
          color: meter.fillColor
        }
      }

      // Day boundary ticks at 1/N … (N-1)/N of the full week width.
      Item {
        id: dayMarkers
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

      // Expected-pace marker: where linear usage "should" be right now.
      // Stronger than day ticks (solid accent, slightly wider, full height).
      Rectangle {
        id: paceMarker
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
}
