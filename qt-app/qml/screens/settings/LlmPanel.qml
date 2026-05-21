// LLM provider settings. All five providers are persisted at once (decision
// #1); the active one is selected here. Per-provider model is an editable
// ComboBox (common models + free entry, decision #6); Ollama populates its list
// from `<base_url-host>/api/tags`. API keys never live in `draft` — they go
// straight to the keyring via SettingsStore.setSecret (decision #4); the panel
// only sees `has_key`. "Проверить ключ" probes via POST /settings/test.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant

ScrollView {
    id: panel
    property var scr
    clip: true
    contentWidth: availableWidth

    // Provider id currently being edited (mirrors draft.llm.active).
    property string provider: "anthropic"
    readonly property bool keyless: provider === "ollama"

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
        // editable ComboBox: seed list + current text
        var list = (commonModels[provider] || []).slice()
        if (c.model && list.indexOf(c.model) === -1) list.unshift(c.model)
        modelBox.model = list
        modelBox.editText = c.model || ""
        maxTokensSpin.value = c.max_tokens || 4096
        baseUrlField.text = c.base_url || ""
        keyField.text = ""
        if (keyless) fetchOllamaTags()
    }

    function load() {
        var active = (scr.draft.llm && scr.draft.llm.active) || "anthropic"
        provider = active
        providerBox.currentIndex = providerBox.indexOfValue(active)
        loadProvider()
    }
    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

    // Ollama model discovery — query the local daemon directly (keyless, no
    // sidecar route). Derives the host from the configured base_url.
    function fetchOllamaTags() {
        var base = (cfg().base_url || "http://localhost:11434/v1")
        var host = base.replace(/\/v1\/?$/, "").replace(/\/$/, "")
        var xhr = new XMLHttpRequest()
        xhr.onreadystatechange = function () {
            if (xhr.readyState !== XMLHttpRequest.DONE) return
            if (xhr.status !== 200) return
            try {
                var data = JSON.parse(xhr.responseText)
                var names = (data.models || []).map(function (m) { return m.name })
                if (names.length === 0) return
                var cur = cfg().model || ""
                if (cur && names.indexOf(cur) === -1) names.unshift(cur)
                modelBox.model = names
                modelBox.editText = cur || names[0]
            } catch (e) { /* leave the common-model fallback in place */ }
        }
        xhr.open("GET", host + "/api/tags")
        xhr.send()
    }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 16

        Label {
            Layout.margins: 16
            Layout.bottomMargin: 0
            text: qsTr("LLM-провайдер")
            font.pixelSize: 18
            font.bold: true
        }

        GroupBox {
            title: qsTr("Активный провайдер")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            ComboBox {
                id: providerBox
                anchors.left: parent.left
                anchors.right: parent.right
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

        GroupBox {
            title: qsTr("Параметры провайдера")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            ColumnLayout {
                anchors.fill: parent
                spacing: 12

                Label { text: qsTr("Модель"); opacity: 0.7 }
                ComboBox {
                    id: modelBox
                    Layout.fillWidth: true
                    editable: true
                    onAccepted: { panel.cfg().model = editText; scr.touch() }
                    onActivated: { panel.cfg().model = currentText; scr.touch() }
                    // also capture free-text edits that don't fire onAccepted
                    Component.onCompleted: editText = panel.cfg().model || ""
                    onEditTextChanged: { panel.cfg().model = editText; scr.touch() }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Label { text: qsTr("Макс. токенов"); Layout.fillWidth: true }
                    SpinBox {
                        id: maxTokensSpin
                        from: 256; to: 200000; stepSize: 256
                        editable: true
                        onValueModified: { panel.cfg().max_tokens = value; scr.touch() }
                    }
                }

                Label { text: qsTr("Базовый URL (пусто = стандартный)"); opacity: 0.7 }
                TextField {
                    id: baseUrlField
                    Layout.fillWidth: true
                    placeholderText: qsTr("По умолчанию для провайдера")
                    onEditingFinished: {
                        panel.cfg().base_url = text.trim().length ? text.trim() : null
                        scr.touch()
                    }
                }
            }
        }

        GroupBox {
            title: qsTr("API-ключ")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.bottomMargin: 16
            visible: !panel.keyless
            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    opacity: 0.7
                    // depend on SettingsStore.snapshot so has_key updates live
                    text: (SettingsStore.snapshot,
                           SettingsStore.providerCfg(panel.provider).has_key)
                          ? qsTr("Ключ сохранён. Введите новый, чтобы заменить.")
                          : qsTr("Ключ не задан.")
                }
                RowLayout {
                    Layout.fillWidth: true
                    TextField {
                        id: keyField
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: qsTr("Вставьте API-ключ")
                    }
                    Button {
                        text: qsTr("Сохранить ключ")
                        enabled: keyField.text.trim().length > 0
                        onClicked: SettingsStore.setSecret(
                            panel.provider, keyField.text.trim(),
                            function (ok, e) {
                                keyResult.text = ok ? qsTr("Ключ сохранён")
                                                    : qsTr("Ошибка: %1").arg(e)
                                if (ok) keyField.text = ""
                            })
                    }
                    Button {
                        text: qsTr("Удалить")
                        enabled: (SettingsStore.snapshot,
                                  SettingsStore.providerCfg(panel.provider).has_key)
                        onClicked: SettingsStore.setSecret(
                            panel.provider, null,
                            function (ok, e) {
                                keyResult.text = ok ? qsTr("Ключ удалён")
                                                    : qsTr("Ошибка: %1").arg(e)
                            })
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Button {
                        id: testBtn
                        text: qsTr("Проверить ключ")
                        onClicked: {
                            keyResult.text = qsTr("Проверка…")
                            SettingsStore.testProvider(
                                panel.provider,
                                function (ok, e) {
                                    keyResult.text = ok
                                        ? qsTr("✓ Ключ работает")
                                        : qsTr("✗ %1").arg(e)
                                })
                        }
                    }
                    Label {
                        id: keyResult
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        opacity: 0.8
                    }
                }
            }
        }

        // Ollama: keyless connectivity test.
        GroupBox {
            title: qsTr("Подключение")
            Layout.fillWidth: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.bottomMargin: 16
            visible: panel.keyless
            RowLayout {
                anchors.fill: parent
                Button {
                    text: qsTr("Проверить подключение")
                    onClicked: {
                        ollamaResult.text = qsTr("Проверка…")
                        SettingsStore.testProvider(
                            "ollama",
                            function (ok, e) {
                                ollamaResult.text = ok
                                    ? qsTr("✓ Ollama доступна")
                                    : qsTr("✗ %1").arg(e)
                            })
                    }
                }
                Label { id: ollamaResult; Layout.fillWidth: true; opacity: 0.8 }
            }
        }
    }
}
