import QtQuick
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "codechap.grok-super-usage"
  ipcTarget: "codechap.grok-super-usage"
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
  readonly property var accounts: hostWidget && Array.isArray(hostWidget.accounts) ? hostWidget.accounts : []
  readonly property double nowMs: hostWidget ? Number(hostWidget.nowMs) : Date.now()
  readonly property int prepaidCredits: hostWidget ? Number(hostWidget.prepaidCredits) || 0 : 0
  readonly property bool onCredits: primaryPercent >= 1.0 && prepaidCredits > 0
  readonly property bool multiAccount: accounts.length > 1

  readonly property bool grokHasData: {
    var accs = root.accounts
    if (accs && accs.length) {
      for (var i = 0; i < accs.length; i++) {
        if (Number(accs[i] && accs[i].rateLimitPercent) >= 0) return true
        if (String((accs[i] && accs[i].usageStatusText) || "") !== "") return true
      }
    }
    return rawPrimaryPercent >= 0
  }
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

  readonly property string weeklyTitle: tierLabel !== "" ? tierLabel : "Grok"
  readonly property string grokRebillLabel: root.formatRebillLabel(subscriptionPeriodEnd, subscriptionCancelsAtEnd)
  readonly property string heroMeta: {
    if (root.multiAccount) return root.accounts.length + " SuperGrok logins"
    if (usageStatusText !== "") return usageStatusText
    if (grokRebillLabel !== "") return grokRebillLabel
    return "\u00A0"
  }
  readonly property real grokMetaOpacity: {
    if (root.multiAccount) return 1
    if (usageStatusText !== "") return 1
    return grokRebillLabel !== "" ? 1 : 0
  }

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

  function formatRebillLabel(iso, cancels) {
    var when = root.parseResetWhen(iso)
    if (!when) return ""
    var verb = cancels === true ? "expires" : "renews"
    return verb + " " + root.shortMonthName(when) + " " + when.getDate()
      + ", " + when.getFullYear()
  }

  function setCenterHoverRevealSuppressed(value) {
    if (root.bar && typeof root.bar.setCenterHoverRevealSuppressed === "function")
      root.bar.setCenterHoverRevealSuppressed(value)
    else if (root.bar && "centerHoverRevealSuppressed" in root.bar)
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
    root.settingsOpen = false
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

  function openSettings() {
    setCenterHoverRevealSuppressed(false)
    root.controller.hide()
    root.settingsOpen = true
  }

  function closeSettings() { root.settingsOpen = false }

  function snapshotCurrentLogin() {
    if (hostWidget && typeof hostWidget.snapshotCurrentLogin === "function")
      hostWidget.snapshotCurrentLogin()
  }

  function forgetSavedLogin(path) {
    if (hostWidget && typeof hostWidget.forgetSavedLogin === "function")
      hostWidget.forgetSavedLogin(path)
  }

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
    if (root.settingsOpen) closeSettings()
    else if (root.opened) close()
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

  function openConsole() {
    Qt.openUrlExternally("https://console.x.ai/")
    root.close()
  }

  function openBilling() {
    Qt.openUrlExternally("https://grok.com/?_s=billing")
    root.close()
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
    contentHeight: panel.fittedContentHeight(column.implicitHeight, Style.space(900))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onActivateRequested: root.refresh()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) {
        if (t === "r" || t === "R") root.refresh()
        if (t === "s" || t === "S") root.openSettings()
        if (t === "c" || t === "C") root.openConsole()
      }

      Flickable {
        id: usageFlick
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        // Do not steal mouse from toggles/buttons; wheel still scrolls.
        interactive: false

        WheelHandler {
          enabled: usageFlick.contentHeight > usageFlick.height + 2
          onWheel: function(event) {
            var next = usageFlick.contentY - event.angleDelta.y
            var maxY = Math.max(0, usageFlick.contentHeight - usageFlick.height)
            usageFlick.contentY = Math.max(0, Math.min(maxY, next))
            event.accepted = true
          }
        }

      Column {
        id: column
        width: parent.width
        spacing: Style.space(12)

        Column {
          visible: root.grokHasData || root.usageStatusText !== "" || root.accounts.length > 0
          width: parent.width
          spacing: Style.space(12)

          PlanHeader {
            width: parent.width
            title: root.weeklyTitle
            meta: root.heroMeta
            metaOpacity: root.grokMetaOpacity
            foreground: root.foreground
            dim: root.dim
            fontFamily: root.fontFamily
            onSettingsClicked: root.openSettings()
            onConsoleClicked: root.openConsole()
          }

          Repeater {
            model: root.accounts.length > 0 ? root.accounts : (root.grokHasData || root.usageStatusText !== "" ? [{
              rateLimitPercent: root.rawPrimaryPercent,
              rateLimitResetAt: root.resetAt,
              rateLimitPeriodStart: root.periodStart,
              categories: root.categories,
              prepaidCredits: root.prepaidCredits,
              usageStatusText: root.usageStatusText,
              authHelpText: root.authHelpText,
              accountEmail: hostWidget ? String(hostWidget.grokLoginEmail || "") : "",
              accountName: hostWidget ? String(hostWidget.grokLoginName || "") : ""
            }] : [])

            Column {
              required property var modelData
              required property int index
              width: column.width
              spacing: Style.space(12)

              PanelSeparator {
                visible: index > 0
                foreground: root.foreground
              }

              UsageSection {
                width: parent.width
                usage: modelData
                showAccountLabel: root.multiAccount
                nowMs: root.nowMs
                paceAlarmEnabled: root.paceAlarmEnabled
                paceAlarmFloor: root.paceAlarmFloor
                foreground: root.foreground
                urgent: root.urgent
                dim: root.dim
                underPaceColor: root.underPaceColor
                overPaceColor: root.overPaceColor
                paceMarkerColor: root.paceMarkerColor
                track: root.track
                fontFamily: root.fontFamily
              }
            }
          }

          Column {
            visible: root.billingUsedLabel !== "" || root.billingHelpText !== ""
            width: parent.width
            spacing: Style.space(10)

            PanelSeparator {
              visible: root.accounts.length > 0 || root.primaryPercent >= 0
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
                textFormat: Text.PlainText
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
                textFormat: Text.PlainText
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
              textFormat: Text.PlainText
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }
          }
        }

        Row {
          width: parent.width
          spacing: Style.space(10)

          BorderSurface {
            width: (parent.width - parent.spacing) / 2
            implicitHeight: Style.space(40)
            color: "transparent"
            borderSpec: Border.controlSpec("normal", root.foreground, Color.accent)
            radius: Style.cornerRadius

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.openConsole()
            }

            Row {
              anchors.centerIn: parent
              spacing: Style.space(8)

              Text {
                text: "󰏌"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.icon
                anchors.verticalCenter: parent.verticalCenter
              }

              Text {
                text: "xAI Console"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                anchors.verticalCenter: parent.verticalCenter
              }
            }
          }

          BorderSurface {
            width: (parent.width - parent.spacing) / 2
            implicitHeight: Style.space(40)
            color: "transparent"
            borderSpec: Border.controlSpec("normal", root.foreground, Color.accent)
            radius: Style.cornerRadius

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.openBilling()
            }

            Row {
              anchors.centerIn: parent
              spacing: Style.space(8)

              Text {
                text: "󰋭"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.icon
                anchors.verticalCenter: parent.verticalCenter
              }

              Text {
                text: "Billing"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                anchors.verticalCenter: parent.verticalCenter
              }
            }
          }
        }

      }
      }
    }
  }

  SettingsOverlay {
    opened: root.settingsOpen
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
    accounts: root.accounts
    grokLoginEmail: hostWidget ? String(hostWidget.grokLoginEmail || "") : ""
    grokLoginName: hostWidget ? String(hostWidget.grokLoginName || "") : ""
    onClosed: root.closeSettings()
    onFlagChanged: function(key, on) { root.setFlag(key, on) }
    onKeyPathCommitted: function(path) { root.saveManagementKeyPath(path) }
    onSaveCurrentLogin: root.snapshotCurrentLogin()
    onForgetSavedLogin: function(path) { root.forgetSavedLogin(path) }
  }
}
