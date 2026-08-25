import QtQuick
import qs.Commons
import qs.Ui

Column {
  id: form
  property color foreground: Color.foreground
  property color dim: Color.foreground
  property string fontFamily: Style.font.family
  property bool showWeeklyUsage: true
  property bool showApiBilling: true
  property bool paceAlarmEnabled: false
  property string managementKeyPath: ""
  property bool billingHasData: false
  property string billingLabel: ""
  property string billingHelpText: ""
  signal flagChanged(string key, bool on)
  signal keyPathCommitted(string path)

  width: parent ? parent.width : implicitWidth
  spacing: Style.space(10)

  Toggle {
    width: parent.width
    label: "Show weekly usage"
    description: "Show weekly percent and reset on the bar. The panel always has the full breakdown."
    checked: form.showWeeklyUsage
    foreground: form.foreground
    fontFamily: form.fontFamily
    onClicked: form.flagChanged("showWeeklyUsage", !form.showWeeklyUsage)
  }

  Toggle {
    width: parent.width
    label: "Show API billing"
    description: "Show the API invoice amount on the bar. The panel always has the bill."
    checked: form.showApiBilling
    foreground: form.foreground
    fontFamily: form.fontFamily
    onClicked: form.flagChanged("showApiBilling", !form.showApiBilling)
  }

  Toggle {
    width: parent.width
    label: "Pace warning"
    description: "Turn the bar red when weekly usage is ahead of an even burn through the week. Off by default."
    checked: form.paceAlarmEnabled
    foreground: form.foreground
    fontFamily: form.fontFamily
    onClicked: form.flagChanged("paceAlarm", !form.paceAlarmEnabled)
  }

  Text {
    width: parent.width
    text: "Management key file"
    color: form.foreground
    font.family: form.fontFamily
    font.pixelSize: Style.font.body
    font.bold: true
  }

  Text {
    width: parent.width
    text: "Path to a chmod 600 key file. Pasting a key writes it to a private file in this plugin folder; the path is what gets saved."
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WordWrap
  }

  TextField {
    width: parent.width
    text: form.managementKeyPath
    placeholderText: "~/dev/XAI-MGMT-KEY.txt"
    password: {
      var t = String(text || "").trim()
      return t.indexOf("xai-") === 0 || t.indexOf("xai_") === 0
    }
    foreground: form.foreground
    font.family: form.fontFamily
    font.pixelSize: Style.font.body
    onEditingFinished: form.keyPathCommitted(text)
  }

  Text {
    width: parent.width
    visible: form.billingHasData
    text: "Current API bill: " + form.billingLabel
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
  }

  Text {
    width: parent.width
    visible: !form.billingHasData && form.billingHelpText !== ""
    text: form.billingHelpText
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WordWrap
  }
}
