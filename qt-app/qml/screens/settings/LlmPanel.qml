// LLM provider settings. Edits draft.llm; secrets go through SettingsStore.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

ScrollView {
    id: panel
    property var scr
    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
    clip: true
    contentWidth: availableWidth

    property string provider: "anthropic"
    readonly property bool keyless: provider === "ollama"
    property bool keyVisible: false
    readonly property bool hasProviderKey: (SettingsStore.snapshot,
                                            SettingsStore.providerCfg(provider).has_key)

    readonly property var providers: [
        { id: "anthropic", label: "Anthropic (Claude)" },
        { id: "openai",    label: "OpenAI (ChatGPT)" },
        { id: "gemini",    label: "Google (Gemini)" },
        { id: "mistral",   label: "Mistral" },
        { id: "ollama",    label: "Ollama (локально)" }
    ]
    readonly property var commonModels: ({
        "anthropic": ["claude-sonnet-4-6", "claude-opus-4-7", "claude-haiku-4-5"],
        "openai":    ["gpt-4o", "gpt-4o-mini", "o3"],
        "gemini":    ["gemini-2.5-pro", "gemini-2.5-flash"],
        "mistral":   ["mistral-large-latest", "mistral-small-latest"],
        "ollama":    ["llama3.1:8b", "qwen2.5:14b", "mistral"]
    })

    function llm() { return scr.draft.llm || (scr.draft.llm = {}) }
    function cfg() {
        var l = llm()
        if (!l[provider]) l[provider] = { model: "", max_tokens: 4096, base_url: null }
        return l[provider]
    }

    function loadProvider() {
        var c = cfg()
        var list = (commonModels[provider] || []).slice()
        if (c.model && list.indexOf(c.model) === -1) list.unshift(c.model)
        modelBox.model = list
        modelBox.editText = c.model || ""
        maxTokensSpin.value = c.max_tokens || 4096
        baseUrlField.text = c.base_url || ""
        keyField.text = ""
        keyVisible = false
        if (keyless) fetchOllamaTags()
    }

    function maskedKeyLabel() {
        switch (provider) {
        case "anthropic": return "sk-ant-api03-" + "•".repeat(36)
        case "openai": return "sk-" + "•".repeat(42)
        case "gemini": return "AIza" + "•".repeat(34)
        case "mistral": return "•".repeat(40)
        default: return "•".repeat(40)
        }
    }

    function load() {
        var active = (scr.draft.llm && scr.draft.llm.active) || "anthropic"
        provider = active
        providerBox.currentIndex = providerBox.indexOfValue(active)
        loadProvider()
    }
    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

    function fetchOllamaTags() {
        var base = (cfg().base_url || "http://localhost:11434/v1")
        var host = base.replace(/\/v1\/?$/, "").replace(/\/$/, "")
        var xhr = new XMLHttpRequest()
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE || xhr.status !== 200) return
            try {
                var data = JSON.parse(xhr.responseText)
                var names = (data.models || []).map(function (m) { return m.name })
                if (names.length === 0) return
                var cur = cfg().model || ""
                if (cur && names.indexOf(cur) === -1) names.unshift(cur)
                modelBox.model = names
                modelBox.editText = cur || names[0]
            } catch (e) {}
        }
        xhr.open("GET", host + "/api/tags")
        xhr.send()
    }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 0

        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 32
            text: qsTr("LLM-провайдер")
            font.family: Theme.fontSerif
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
            text: qsTr("meety использует большую модель для превращения транскрипта в структурированный протокол.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        SettingsRow {
            title: qsTr("Провайдер")
            help: qsTr("Активная модель для генерации протоколов.")
            MeetyComboBox {
                id: providerBox
                Layout.fillWidth: true
                textRole: "label"
                valueRole: "id"
                model: panel.providers
                onActivated: {
                    panel.provider = currentValue
                    panel.llm().active = currentValue
                    scr.touch()
                    panel.loadProvider()
                }
            }
        }

        SettingsRow {
            title: qsTr("Модель")
            help: qsTr("Можно выбрать популярную модель или ввести название вручную.")
            MeetyComboBox {
                id: modelBox
                Layout.fillWidth: true
                editable: true
                onAccepted: { panel.cfg().model = editText; scr.touch() }
                onActivated: { panel.cfg().model = currentText; scr.touch() }
                onEditTextChanged: { panel.cfg().model = editText; scr.touch() }
            }
        }

        SettingsRow {
            title: qsTr("Макс. токенов")
            help: qsTr("Лимит ответа модели для одного протокола.")
            MeetySpinBox {
                id: maxTokensSpin
                Layout.fillWidth: true
                from: 256; to: 200000; stepSize: 256
                editable: true
                onValueModified: { panel.cfg().max_tokens = value; scr.touch() }
            }
        }

        SettingsRow {
            title: qsTr("Базовый URL")
            help: qsTr("Оставьте пустым, чтобы использовать стандартный endpoint провайдера.")
            MeetyField {
                id: baseUrlField
                Layout.fillWidth: true
                placeholderText: qsTr("По умолчанию для провайдера")
                onEditingFinished: {
                    panel.cfg().base_url = text.trim().length ? text.trim() : null
                    scr.touch()
                }
            }
        }

        SettingsRow {
            visible: !panel.keyless
            title: qsTr("API-ключ")
            contentMaximumWidth: 760
            help: panel.hasProviderKey
                  ? qsTr("Хранится в системном Keychain. Введите новый, чтобы заменить.")
                  : qsTr("Ключ не задан.")
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 38
                    radius: Theme.rMd
                    color: Theme.paper
                    border.width: 1
                    border.color: keyField.activeFocus ? Theme.ink3 : Theme.rule

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 4
                        spacing: 6

                        TextField {
                            id: keyField
                            Layout.fillWidth: true
                            background: null
                            text: panel.hasProviderKey && !activeFocus ? panel.maskedKeyLabel() : ""
                            echoMode: panel.keyVisible ? TextInput.Normal : TextInput.Password
                            readOnly: panel.hasProviderKey && !activeFocus
                            selectByMouse: true
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBodyLg
                            color: readOnly ? Theme.ink3 : Theme.ink
                            placeholderText: panel.hasProviderKey
                                             ? qsTr("Введите новый ключ, чтобы заменить сохранённый")
                                             : qsTr("Вставьте API-ключ")
                            placeholderTextColor: Theme.ink4
                            selectionColor: Theme.accentTint
                            selectedTextColor: Theme.ink
                            onActiveFocusChanged: {
                                if (activeFocus && panel.hasProviderKey && text === panel.maskedKeyLabel())
                                    text = ""
                            }
                        }
                        MeetyIconButton {
                            iconName: panel.keyVisible ? "eye-off" : "eye"
                            iconSize: 15
                            enabled: keyField.text.length > 0
                            onClicked: panel.keyVisible = !panel.keyVisible
                            MeetyToolTip {
                                text: panel.keyVisible ? qsTr("Скрыть ключ") : qsTr("Показать ключ")
                                visible: parent.hovered
                            }
                        }
                    }
                }

                MeetyButton {
                    Layout.alignment: Qt.AlignRight
                    text: panel.hasProviderKey ? qsTr("Заменить") : qsTr("Сохранить")
                    enabled: keyField.text.trim().length > 0
                             && keyField.text !== panel.maskedKeyLabel()
                    onClicked: SettingsStore.setSecret(
                        panel.provider, keyField.text.trim(),
                        function (ok, e) {
                            keyResult.text = ok ? qsTr("Ключ сохранён")
                                                : qsTr("Ошибка: %1").arg(e)
                            if (ok) {
                                keyField.text = ""
                                panel.keyVisible = false
                            }
                        })
                }
            }
        }

        SettingsRow {
            visible: !panel.keyless
            title: qsTr("Проверка ключа")
            help: qsTr("Быстрый запрос к выбранному провайдеру.")
            dividerVisible: false
            MeetyButton {
                text: qsTr("Проверить ключ")
                enabled: panel.hasProviderKey || keyField.text.trim().length > 0
                onClicked: {
                    keyResult.text = qsTr("Проверка…")
                    SettingsStore.testProvider(
                        panel.provider,
                        function (ok, e) {
                            keyResult.text = ok
                                ? qsTr("Ключ работает")
                                : qsTr("%1").arg(e)
                        })
                }
            }
            MeetyButton {
                variant: "ghost"
                text: qsTr("Удалить")
                enabled: panel.hasProviderKey
                onClicked: SettingsStore.setSecret(
                    panel.provider, null,
                    function (ok, e) {
                        keyResult.text = ok ? qsTr("Ключ удалён")
                                            : qsTr("Ошибка: %1").arg(e)
                    })
            }
            Text {
                id: keyResult
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.ink3
            }
        }

        SettingsRow {
            visible: panel.keyless
            title: qsTr("Подключение")
            help: qsTr("Ollama работает локально и не требует API-ключа.")
            dividerVisible: false
            MeetyButton {
                text: qsTr("Проверить подключение")
                onClicked: {
                    ollamaResult.text = qsTr("Проверка…")
                    SettingsStore.testProvider(
                        "ollama",
                        function (ok, e) {
                            ollamaResult.text = ok
                                ? qsTr("Ollama доступна")
                                : qsTr("%1").arg(e)
                        })
                }
            }
            Text {
                id: ollamaResult
                Layout.fillWidth: true
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.ink3
            }
        }
    }
}
