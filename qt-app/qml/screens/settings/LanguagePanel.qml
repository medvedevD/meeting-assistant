// Interface language. Not a server setting: the choice goes straight into
// QSettings through `appLanguage`, so "Сохранить" does not apply to it. It takes
// effect after a restart, announced by SettingsScreen's restart banner.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

ScrollView {
    id: panel
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
    clip: true
    contentWidth: availableWidth

    ColumnLayout {
        width: panel.availableWidth
        spacing: 0

        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 32
            text: qsTr("Язык")
            font.family: Theme.serifFor(text)
            renderType: Theme.renderTypeFor(font.family)
            font.pixelSize: 26
            font.weight: Theme.wMedium
            font.letterSpacing: 0
            color: Theme.ink
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 4
            Layout.bottomMargin: 28
            text: qsTr("Язык меню, подсказок и сообщений. Язык протокола задаёт его шаблон.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        SettingsRow {
            dividerVisible: false
            title: qsTr("Язык интерфейса")
            help: qsTr("Применится после перезапуска приложения.")
            MeetyComboBox {
                id: languageBox
                objectName: "languageBox"
                Layout.fillWidth: true
                textRole: "label"
                valueRole: "code"
                // Language names stay in their own language, so a reader who
                // cannot read the current interface can still find theirs.
                model: [
                    { code: "system", label: qsTr("Как в системе") },
                    { code: "ru",     label: "Русский" },
                    { code: "en",     label: "English" }
                ]
                Component.onCompleted: currentIndex = indexOfValue(appLanguage.choice)
                onActivated: appLanguage.choice = currentValue
            }
        }
    }
}
