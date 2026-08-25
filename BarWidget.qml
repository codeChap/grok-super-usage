import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Bar chip: SuperGrok weekly % / reset, plus optional API invoice spend.
// Left click toggles the panel; right click refreshes.
BarWidget {
  id: root
  moduleName: "codechap.grokbar"

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  property double nowMs: Date.now()
  property real primaryPercent: -1
  property string resetAt: ""
  property string periodStart: ""
  property string tierLabel: ""
  property string grokLoginName: ""
  property string grokLoginEmail: ""
  property string subscriptionPeriodEnd: ""
  property bool subscriptionCancelsAtEnd: false
  property string usageStatusText: ""
  property string authHelpText: ""
  property var categories: []
  property bool hasData: false
  property bool refreshing: false
  property bool grokAvailable: false

  property string billingLabel: ""
  property real billingUsd: -1
  property string billingPeriod: ""
  property string billingStatusText: ""
  property string billingHelpText: ""
  property bool billingHasData: false
  property bool billingRefreshing: false
  property bool billingAvailable: false

  readonly property int refreshIntervalSec: Math.max(30, Number(setting("refreshIntervalSec", 300)) || 300)
  readonly property bool showWeeklyUsage: !settings || settings.showWeeklyUsage !== false
  readonly property bool showApiBilling: !settings || settings.showApiBilling !== false
  readonly property bool paceAlarm: !!(settings && settings.paceAlarm === true)

  readonly property real expectedPace: {
    var start = root.parseTimeMs(periodStart)
    var end = root.parseTimeMs(resetAt)
    if (!(start > 0) || !(end > start)) {
      if (!(end > 0)) return -1
      start = end - 7 * 24 * 3600 * 1000
    }
    var frac = (root.nowMs - start) / (end - start)
    if (!isFinite(frac)) return -1
    return Math.max(0, Math.min(1, frac))
  }

  readonly property real displayPercent: primaryPercent
  readonly property bool overPace: expectedPace >= 0 && displayPercent >= 0
    && displayPercent > expectedPace + 0.0001
  readonly property real paceAlarmFloor: {
    var v = Number(setting("paceAlarmFloor", 0.15))
    if (!isFinite(v)) return 0.15
    return Math.max(0, Math.min(1, v))
  }
  // Red when used >= 90%, or (paceAlarm on AND ahead of even-burn AND used >= floor).
  readonly property bool grokAlarming: displayPercent >= 0.9
    || (paceAlarm && overPace && displayPercent >= paceAlarmFloor)
  readonly property bool alarming: grokAlarming
  readonly property bool grokVisible: hasData
  readonly property bool chipVisible: hasData || billingHasData
  readonly property string primaryText: displayPercent >= 0 ? Math.round(displayPercent * 100) + "%" : ""
  readonly property string resetText: {
    if (resetAt === "") return ""
    var ms = new Date(resetAt).getTime() - root.nowMs
    return isFinite(ms) ? root.formatBarDuration(ms) : ""
  }
  readonly property string billingText: root.billingHasData && root.billingLabel !== ""
    ? root.billingLabel : ""

  readonly property string scannerPath: root.fileUrlToPath(Qt.resolvedUrl("grokbar"))
  readonly property string pluginKeyPath: root.fileUrlToPath(Qt.resolvedUrl("management.key"))
  readonly property url iconSource: Qt.resolvedUrl("assets/grok.svg")
  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function fileUrlToPath(url) {
    var text = String(url || "")
    if (text.indexOf("file://") === 0) {
      text = text.slice(7)
      if (text.indexOf("localhost/") === 0)
        text = text.slice(9)
      else if (text.charAt(0) !== "/") {
        var slash = text.indexOf("/")
        if (slash >= 0) text = text.slice(slash)
      }
      try { text = decodeURIComponent(text) } catch (e) {}
      return text
    }
    return text
  }

  function resolvePath(value) {
    var text = String(value || "").trim()
    if (text === "") return ""
    if (text.startsWith("~/"))
      return (Quickshell.env("HOME") || "") + text.slice(1)
    if (text === "~")
      return Quickshell.env("HOME") || ""
    return text
  }

  function looksLikeManagementKey(value) {
    var text = String(value || "").trim()
    if (text.length < 20 || text.length > 256) return false
    if (text.indexOf("/") >= 0 || text.indexOf("\\") >= 0
        || text.indexOf(".") >= 0 || text.indexOf("~") >= 0)
      return false
    if (/\s/.test(text)) return false
    return text.indexOf("xai-") === 0 || text.indexOf("xai_") === 0
  }

  function probeStatus(text) {
    var t = String(text || "").trim()
    if (t === "present" || t === "ready") return "present"
    if (t === "unreadable" || t === "absent") return t
    return ""
  }

  function scannerCommand(probe) {
    var command = [root.scannerPath, "grok"]
    if (probe)
      command.push("--probe")
    var authPath = root.resolvePath(root.setting("authPath", ""))
    if (authPath !== "")
      command.push("--auth", authPath)
    return command
  }

  function billingCommand(probe) {
    var command = [root.scannerPath, "billing"]
    if (probe)
      command.push("--probe")
    var raw = String(root.setting("managementKeyPath", "") || "").trim()
    if (raw === "")
      return command
    if (root.looksLikeManagementKey(raw)) {
      root.stashPastedKey(raw)
      return command
    }
    var keyFile = root.resolvePath(raw)
    if (keyFile !== "")
      command.push("--key-file", keyFile)
    return command
  }

  function stashPastedKey(key) {
    if (!key || storeKeyProc.running)
      return
    storeKeyProc._pending = key
    storeKeyProc.command = [root.scannerPath, "store-key", "--out", root.pluginKeyPath]
    storeKeyProc.running = true
  }

  function formatBarDuration(ms) {
    if (!(ms > 0)) return "now"
    var hours = Math.floor(ms / 3600000)
    var days = Math.floor(hours / 24)
    if (days > 0) return days + "d"
    return Math.max(1, hours) + "h"
  }

  function parseTimeMs(value) {
    var text = String(value || "").trim()
    if (text === "") return NaN
    var t = new Date(text).getTime()
    return isFinite(t) ? t : NaN
  }

  function applyScan(data) {
    if (!data || typeof data !== "object") {
      root.hasData = false
      return
    }
    var primary = Number(data.rateLimitPercent)
    if (!isFinite(primary)) primary = -1
    root.primaryPercent = primary
    root.resetAt = String(data.rateLimitResetAt || "")
    root.periodStart = String(data.rateLimitPeriodStart || "")
    root.tierLabel = String(data.tierLabel || "")
    root.grokLoginName = String(data.accountName || "")
    root.grokLoginEmail = String(data.accountEmail || "")
    root.subscriptionPeriodEnd = String(data.subscriptionPeriodEnd || "")
    root.subscriptionCancelsAtEnd = data.subscriptionCancelsAtEnd === true
    root.usageStatusText = String(data.usageStatusText || "")
    root.authHelpText = String(data.authHelpText || "")
    root.categories = Array.isArray(data.categories) ? data.categories : []
    root.hasData = primary >= 0
    root.nowMs = Date.now()
    root.injectPanel()
  }

  function clearUsage() {
    root.primaryPercent = -1
    root.resetAt = ""
    root.periodStart = ""
    root.tierLabel = ""
    root.grokLoginName = ""
    root.grokLoginEmail = ""
    root.subscriptionPeriodEnd = ""
    root.subscriptionCancelsAtEnd = false
    root.usageStatusText = ""
    root.authHelpText = ""
    root.categories = []
    root.hasData = false
  }

  function parseScannerJson(text, label) {
    var raw = String(text || "").trim()
    if (!raw)
      return null
    if (raw.charAt(0) !== "{" && raw.charAt(0) !== "[") {
      console.warn("codechap.grokbar: unexpected " + label + " output (" + raw.length + " bytes)")
      return null
    }
    try {
      return JSON.parse(raw)
    } catch (e) {
      console.warn("codechap.grokbar: bad " + label + " JSON (" + raw.length + " bytes)")
      return null
    }
  }

  function startIfIdle(proc, command) {
    if (!proc) return
    if (command)
      proc.command = command
    if (!proc.running)
      proc.running = true
  }

  function probeGrok() {
    root.startIfIdle(presenceProbe, root.scannerCommand(true))
  }

  function probeBilling() {
    root.startIfIdle(billingProbe, root.billingCommand(true))
  }

  function applyBilling(data) {
    if (!data || typeof data !== "object" || data.ready !== true) {
      if (data && data.usageStatusText)
        root.billingStatusText = String(data.usageStatusText || "")
      if (data && data.authHelpText)
        root.billingHelpText = String(data.authHelpText || "")
      root.injectPanel()
      return
    }
    var usd = Number(data.amountUsd)
    root.billingUsd = isFinite(usd) ? usd : -1
    root.billingLabel = String(data.amountLabel || "")
    root.billingPeriod = String(data.period || "")
    root.billingStatusText = String(data.usageStatusText || "")
    root.billingHelpText = String(data.authHelpText || "")
    root.billingHasData = root.billingLabel !== ""
    root.billingAvailable = true
    root.injectPanel()
  }

  function refreshBilling() {
    if (!root.billingAvailable) return
    root.billingRefreshing = true
    root.startIfIdle(billingScanner, root.billingCommand(false))
  }

  function persistSettings(values) {
    var next = values || {}
    if (next.managementKeyPath !== undefined) {
      var raw = String(next.managementKeyPath || "").trim()
      if (root.looksLikeManagementKey(raw)) {
        root.stashPastedKey(raw)
        delete next.managementKeyPath
      }
    }
    var empty = true
    for (var check in next) { empty = false; break }
    if (empty) return
    var entry = { id: root.moduleName }
    for (var existing in root.settings) if (existing !== "id") entry[existing] = root.settings[existing]
    for (var key in next) entry[key] = next[key]
    root.settings = entry
    if (root.bar && root.bar.shell && typeof root.bar.shell.updateEntryInline === "function")
      root.bar.shell.updateEntryInline(root.moduleName, entry)
  }

  function refresh() {
    if (root.grokAvailable) root.refreshing = true
    if (root.billingAvailable) root.billingRefreshing = true
    root.probeGrok()
    root.probeBilling()
  }

  function refreshUsage() {
    if (!root.grokAvailable) {
      root.clearUsage()
      return
    }
    if (usageScanner.running) return
    root.refreshing = true
    usageScanner.command = root.scannerCommand()
    usageScanner.running = true
  }

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
  }

  function togglePanel() {
    if (panelLoader.item && panelLoader.item.toggle) panelLoader.item.toggle()
  }

  function open() {
    if (panelLoader.item && panelLoader.item.openFromHotkey) panelLoader.item.openFromHotkey()
  }

  function close() {
    if (panelLoader.item && panelLoader.item.close) panelLoader.item.close()
  }

  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  visible: chipVisible
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: {
    var raw = String(root.setting("managementKeyPath", "") || "").trim()
    if (root.looksLikeManagementKey(raw))
      root.stashPastedKey(raw)
  }

  onBarChanged: injectPanel()
  onSettingsChanged: {
    injectPanel()
    var raw = String(root.setting("managementKeyPath", "") || "").trim()
    if (root.looksLikeManagementKey(raw))
      root.stashPastedKey(raw)
    Qt.callLater(function() {
      root.probeGrok()
      root.probeBilling()
    })
  }
  onShowApiBillingChanged: {
    if (!root.showApiBilling) return
    Qt.callLater(function() {
      root.probeBilling()
      if (root.billingAvailable) root.refreshBilling()
    })
  }

  IpcHandler {
    target: "codechap.grokbar"
    function refresh(): string { root.refresh(); return "ok" }
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.togglePanel() }
  }

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  Process {
    id: presenceProbe
    command: root.scannerCommand(true)
    running: false
    stdout: StdioCollector {
      onStreamFinished: {
        var status = root.probeStatus(text)
        if (status === "") return
        if (status === "present") {
          root.grokAvailable = true
          root.refreshUsage()
          return
        }
        if (status === "unreadable") return
        root.grokAvailable = false
        root.clearUsage()
      }
    }
  }

  Process {
    id: usageScanner
    command: root.scannerCommand()
    running: false
    stdout: StdioCollector {
      onStreamFinished: {
        var data = root.parseScannerJson(text, "scanner")
        if (data)
          root.applyScan(data)
      }
    }
    onExited: root.refreshing = false
    stderr: StdioCollector {
      onStreamFinished: if (text.trim() !== "")
        console.warn("codechap.grokbar scanner stderr (" + text.length + " bytes)")
    }
  }

  Process {
    id: billingProbe
    command: root.billingCommand(true)
    running: false
    stdout: StdioCollector {
      onStreamFinished: {
        var status = root.probeStatus(text)
        if (status === "") return
        if (status === "present") {
          root.billingAvailable = true
          root.refreshBilling()
          return
        }
        if (status === "unreadable") return
        root.billingAvailable = false
      }
    }
  }

  Process {
    id: billingScanner
    command: root.billingCommand(false)
    running: false
    stdout: StdioCollector {
      onStreamFinished: {
        var data = root.parseScannerJson(text, "billing")
        if (data)
          root.applyBilling(data)
      }
    }
    onExited: root.billingRefreshing = false
    stderr: StdioCollector {
      onStreamFinished: if (text.trim() !== "")
        console.warn("codechap.grokbar billing stderr (" + text.length + " bytes)")
    }
  }

  Process {
    id: storeKeyProc
    running: false
    stdinEnabled: true
    property string _pending: ""
    onStarted: {
      if (_pending !== "") {
        write(_pending + "\n")
        _pending = ""
      }
    }
    stdout: StdioCollector {
      onStreamFinished: {
        var path = text.trim()
        if (path.indexOf("/") === 0)
          root.persistSettings({ managementKeyPath: path })
      }
    }
    stderr: StdioCollector {
      onStreamFinished: if (text.trim() !== "")
        console.warn("codechap.grokbar store-key failed")
    }
  }

  Timer {
    interval: 5000
    running: !root.grokAvailable
    repeat: true
    triggeredOnStart: true
    onTriggered: {
      root.probeGrok()
      root.probeBilling()
    }
  }

  Timer {
    interval: root.refreshIntervalSec * 1000
    running: root.grokAvailable || root.billingAvailable
    repeat: true
    onTriggered: {
      root.probeGrok()
      root.probeBilling()
    }
  }

  Timer {
    interval: 30000
    running: root.visible || root.opened
    repeat: true
    onTriggered: root.nowMs = Date.now()
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    labelVisible: false
    hasVisualContent: root.chipVisible
    active: root.alarming
    tooltipText: ""
    fixedWidth: {
      if (vertical) return Style.bar.iconSlot
      return Math.ceil(contentRow.implicitWidth + Style.spaceReal(8.75) * 2)
    }
    fixedHeight: vertical ? Style.bar.iconSlot : -1
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton) root.refresh()
      else root.togglePanel()
    }

    Row {
      id: contentRow
      visible: !button.vertical
      anchors.centerIn: parent
      spacing: Style.space(5)

      ThemedGrokIcon {
        anchors.verticalCenter: parent.verticalCenter
      }

      Text {
        visible: root.showWeeklyUsage && root.primaryText !== ""
        anchors.verticalCenter: parent.verticalCenter
        text: root.primaryText
        color: root.grokAlarming ? button.activeColor : button.foreground
        font.family: button.fontFamily
        font.pixelSize: Style.font.bodySmall
        renderType: Text.NativeRendering
      }

      Text {
        visible: root.showWeeklyUsage && root.resetText !== ""
        anchors.verticalCenter: parent.verticalCenter
        text: root.resetText
        color: root.dim
        font.family: button.fontFamily
        font.pixelSize: Style.font.bodySmall
        renderType: Text.NativeRendering
      }

      Row {
        visible: root.showApiBilling && root.billingText !== ""
        spacing: Style.space(5)
        anchors.verticalCenter: parent.verticalCenter

        Text {
          text: root.billingText
          color: button.foreground
          font.family: button.fontFamily
          font.pixelSize: Style.font.bodySmall
          renderType: Text.NativeRendering
        }

        Text {
          text: "API"
          color: root.dim
          font.family: button.fontFamily
          font.pixelSize: Style.font.bodySmall
          renderType: Text.NativeRendering
        }
      }
    }

    ThemedGrokIcon {
      visible: button.vertical && root.chipVisible
      anchors.centerIn: parent
    }
  }

  component ThemedGrokIcon: Item {
    width: Style.bar.iconCanvas
    height: Style.bar.iconCanvas
    implicitWidth: width
    implicitHeight: height
    readonly property int iconSize: Style.bar.iconFont

    Image {
      id: icon
      anchors.centerIn: parent
      width: parent.iconSize
      height: parent.iconSize
      source: root.iconSource
      sourceSize.width: parent.iconSize * 2
      sourceSize.height: parent.iconSize * 2
      fillMode: Image.PreserveAspectFit
      visible: false
      layer.enabled: true
    }

    MultiEffect {
      anchors.fill: icon
      source: icon
      colorization: 1.0
      colorizationColor: root.foreground
    }
  }
}
