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
  property var accounts: []
  property string grokLoginEmail: ""
  property string grokLoginName: ""
  signal flagChanged(string key, bool on)
  signal keyPathCommitted(string path)
  signal saveCurrentLogin()
  signal forgetSavedLogin(string path)

  readonly property var savedAccounts: {
    var accs = form.accounts
    var out = []
    if (!accs || !accs.length) return out
    for (var i = 0; i < accs.length; i++) {
      if (accs[i] && accs[i].saved === true && String(accs[i].savedPath || "") !== "")
        out.push(accs[i])
    }
    return out
  }
  readonly property string liveLoginLabel: {
    var accs = form.accounts
    if (accs && accs.length) {
      for (var i = 0; i < accs.length; i++) {
        var acc = accs[i]
        if (!acc || acc.saved === true) continue
        var email = String(acc.accountEmail || "")
        if (email !== "") return email
        var name = String(acc.accountName || "")
        if (name !== "") return name
        break
      }
    }
    if (form.grokLoginEmail !== "") return form.grokLoginEmail
    if (form.grokLoginName !== "") return form.grokLoginName
    return "the current Grok CLI login"
  }

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
    text: "Grok logins"
    color: form.foreground
    font.family: form.fontFamily
    font.pixelSize: Style.font.body
    font.bold: true
  }

  Text {
    width: parent.width
    text: "Grok CLI only keeps one login. The first scan of a new login keeps a copy. After grok login with another SuperGrok account, the panel keeps the previous weekly block. Remove a saved login you no longer want."
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WordWrap
  }

  Text {
    width: parent.width
    visible: form.liveLoginLabel !== ""
    text: "This login: " + form.liveLoginLabel
    textFormat: Text.PlainText
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
    elide: Text.ElideRight
  }

  Button {
    text: "Save this login"
    bordered: true
    foreground: form.foreground
    fontFamily: form.fontFamily
    onClicked: form.saveCurrentLogin()
  }

  Repeater {
    model: form.savedAccounts

    Item {
      required property var modelData
      width: form.width
      implicitHeight: Math.max(savedLabel.implicitHeight, removeBtn.implicitHeight)

      Text {
        id: savedLabel
        anchors.left: parent.left
        anchors.right: removeBtn.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: String((modelData && (modelData.accountEmail || modelData.accountName)) || "Saved login")
        textFormat: Text.PlainText
        color: form.foreground
        font.family: form.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }

      Button {
        id: removeBtn
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "Remove"
        bordered: true
        foreground: form.foreground
        fontFamily: form.fontFamily
        fontSize: Style.font.caption
        onClicked: form.forgetSavedLogin(String((modelData && modelData.savedPath) || ""))
      }
    }
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
    textFormat: Text.PlainText
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
  }

  Text {
    width: parent.width
    visible: !form.billingHasData && form.billingHelpText !== ""
    text: form.billingHelpText
    textFormat: Text.PlainText
    color: form.dim
    font.family: form.fontFamily
    font.pixelSize: Style.font.caption
    wrapMode: Text.WordWrap
  }
}
