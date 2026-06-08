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
    // Global secret-store disclosure: { kind, state, mechanism, mechanism_detail,
    // path }. Reads SettingsStore.snapshot, so it re-evaluates on refresh.
    readonly property var secretStorage: SettingsStore.secretStorage()
    // Transient message for a failed secret-store operation (unlock/protect/…).
    property string vaultMsg: ""

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

    // Non-secret fingerprint of the stored key from the snapshot: { last4, len }.
    function keyHint() {
        var c = SettingsStore.providerCfg(provider)
        return (c && c.key_hint) ? c.key_hint : null
    }

    // Masked label: provider prefix · bullets · real last 4 chars (from the
    // fingerprint), so the user can tell *which* key is stored without exposing
    // it. Falls back to plain bullets when no fingerprint is available.
    function maskedKeyLabel() {
        var prefix
        switch (provider) {
        case "anthropic": prefix = "sk-ant-api03-"; break
        case "openai":    prefix = "sk-"; break
        case "gemini":    prefix = "AIza"; break
        default:          prefix = ""
        }
        var h = keyHint()
        var last4 = (h && h.last4) ? h.last4 : ""
        return prefix + "•".repeat(12) + last4
    }

    // Localized label for the concrete OS keystore (backend sends a stable id).
    function mechanismLabel(m, detail) {
        switch (m) {
        case "apple_keychain":             return qsTr("Связка ключей macOS")
        case "windows_credential_manager": return qsTr("Диспетчер учётных данных Windows")
        case "secret_service":             return detail
                                                  ? qsTr("Secret Service (%1)").arg(detail)
                                                  : qsTr("Secret Service")
        case "kwallet":                    return qsTr("KDE KWallet")
        default:                           return ""
        }
    }
    function _parentDir(path) {
        var clean = (path || "").replace(/\/+$/, "")
        var idx = clean.lastIndexOf("/")
        return idx > 0 ? clean.slice(0, idx) : (idx === 0 ? "/" : "")
    }
    function _fileUrl(path) {
        if (!path || path.length === 0) return ""
        return path.charAt(0) === "/" ? "file://" + path : "file:///" + path
    }
    function openSecretFolder() {
        var dir = _parentDir(panel.secretStorage.path || "")
        if (dir.length > 0) Qt.openUrlExternally(_fileUrl(dir))
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

        // Disclosure card: where API keys live at rest — plain-language state
        // plus the concrete OS keystore. Warn-tinted for the plaintext fallback.
        Rectangle {
            id: storeCard
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.bottomMargin: 20
            readonly property string kind: panel.secretStorage.kind || "keyring"
            readonly property bool plaintext: kind === "plaintext"
            readonly property bool vaultLocked: kind === "vault"
                                                && panel.secretStorage.state === "locked"
            readonly property bool vaultUnlocked: kind === "vault" && !vaultLocked
            // Keystore is back but an old passphrase vault still holds keys.
            readonly property bool pendingVault: kind === "keyring"
                                                 && panel.secretStorage.pending_migration === "vault"
            readonly property bool caution: plaintext || vaultLocked
            implicitHeight: cardCol.implicitHeight + 28
            radius: Theme.rLg
            color: Theme.paperSub
            border.width: 1
            border.color: caution ? Theme.warn : Theme.rule

            ColumnLayout {
                id: cardCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: 18
                anchors.rightMargin: 18
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 14

                    MeetyIcon {
                        name: storeCard.plaintext ? "storage" : "key"
                        size: 22
                        color: storeCard.caution ? Theme.warn : Theme.accent
                        Layout.alignment: Qt.AlignVCenter
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 3
                        Text {
                            Layout.fillWidth: true
                            text: {
                                var s = panel.secretStorage
                                switch (s.kind) {
                                case "keyring":
                                    var ml = panel.mechanismLabel(s.mechanism, s.mechanism_detail)
                                    return ml ? qsTr("Системный сейф") + "  ·  " + ml
                                              : qsTr("Системный сейф")
                                case "vault":
                                    return s.state === "locked" ? qsTr("Хранилище заблокировано")
                                                                : qsTr("Под паролем")
                                case "plaintext": return qsTr("Открытый файл")
                                default:          return qsTr("Хранилище ключей")
                                }
                            }
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBodyLg
                            font.weight: Theme.wSemiBold
                            color: Theme.ink
                            wrapMode: Text.WordWrap
                        }
                        Text {
                            Layout.fillWidth: true
                            text: {
                                var s = panel.secretStorage
                                switch (s.kind) {
                                case "keyring":   return qsTr("Ключи защищены вашей учётной записью.")
                                case "plaintext": return qsTr("Любой с доступом к диску прочитает ключи. Файл: %1").arg(s.path || "")
                                case "vault":     return s.state === "locked"
                                                  ? qsTr("Введите пароль, чтобы получить доступ к ключам.")
                                                  : qsTr("Ключи зашифрованы вашим паролем.")
                                default:          return ""
                                }
                            }
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBody
                            color: Theme.ink3
                            wrapMode: Text.WordWrap
                        }
                    }

                    // plaintext actions
                    MeetyButton {
                        variant: "ghost"
                        text: qsTr("Открыть папку")
                        visible: storeCard.plaintext
                        onClicked: panel.openSecretFolder()
                        Layout.alignment: Qt.AlignVCenter
                    }
                    MeetyButton {
                        variant: "accent"
                        text: qsTr("Защитить паролем")
                        visible: storeCard.plaintext
                        onClicked: { passphraseDialog.mode = "create"; passphraseDialog.openClear() }
                        Layout.alignment: Qt.AlignVCenter
                    }
                    // unlocked-vault actions
                    MeetyButton {
                        variant: "ghost"
                        text: qsTr("Сменить пароль")
                        visible: storeCard.vaultUnlocked
                        onClicked: { passphraseDialog.mode = "change"; passphraseDialog.openClear() }
                        Layout.alignment: Qt.AlignVCenter
                    }
                    MeetyButton {
                        variant: "ghost"
                        text: qsTr("Заблокировать")
                        visible: storeCard.vaultUnlocked
                        onClicked: SettingsStore.lockSecretStore(function (ok, e) {
                            panel.vaultMsg = ok ? "" : e
                        })
                        Layout.alignment: Qt.AlignVCenter
                    }
                }

                // locked-vault: inline unlock
                RowLayout {
                    Layout.fillWidth: true
                    visible: storeCard.vaultLocked
                    spacing: 8

                    MeetyField {
                        id: unlockField
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: qsTr("Пароль хранилища")
                        onAccepted: if (text.length > 0) unlockBtn.clicked()
                    }
                    MeetyButton {
                        id: unlockBtn
                        variant: "accent"
                        text: qsTr("Разблокировать")
                        enabled: unlockField.text.length > 0
                        onClicked: SettingsStore.unlockSecretStore(unlockField.text, function (ok, e) {
                            panel.vaultMsg = ok ? "" : qsTr("Неверный пароль")
                            if (ok) unlockField.text = ""
                        })
                    }
                    MeetyButton {
                        variant: "ghost"
                        text: qsTr("Забыли пароль?")
                        onClicked: resetDialog.open()
                        Layout.alignment: Qt.AlignVCenter
                    }
                }

                // keystore is back: offer to fold an orphaned vault into it
                RowLayout {
                    Layout.fillWidth: true
                    visible: storeCard.pendingVault
                    spacing: 8

                    MeetyField {
                        id: migrateField
                        Layout.fillWidth: true
                        echoMode: TextInput.Password
                        placeholderText: qsTr("Пароль хранилища")
                        onAccepted: if (text.length > 0) migrateBtn.clicked()
                    }
                    MeetyButton {
                        id: migrateBtn
                        variant: "accent"
                        text: qsTr("Перенести в системный сейф")
                        enabled: migrateField.text.length > 0
                        onClicked: SettingsStore.migrateSecretStore(migrateField.text, function (ok, e) {
                            panel.vaultMsg = ok ? "" : qsTr("Неверный пароль")
                            if (ok) migrateField.text = ""
                        })
                    }
                }

                Text {
                    Layout.fillWidth: true
                    visible: panel.vaultMsg.length > 0
                    text: panel.vaultMsg
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsBody
                    color: Theme.warn
                }
            }
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
            help: !panel.hasProviderKey
                  ? qsTr("Ключ не задан.")
                  : SettingsStore.secretStorage().kind === "plaintext"
                    ? qsTr("⚠ Хранится в открытом файле — системное хранилище недоступно. Введите новый, чтобы заменить.")
                    : qsTr("Хранится в системном сейфе. Введите новый, чтобы заменить.")
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                // Read-only fingerprint of the stored key: provider prefix · dots ·
                // real last 4 chars · length. Shows *which* key is in use, in a
                // plain (non-password) mono line. The secret itself never leaves
                // the backend — only the fingerprint does.
                Text {
                    visible: panel.hasProviderKey
                    Layout.fillWidth: true
                    text: {
                        var h = panel.keyHint()
                        var len = (h && h.len) ? qsTr("  ·  %1 симв.").arg(h.len) : ""
                        return panel.maskedKeyLabel() + len
                    }
                    elide: Text.ElideRight
                    font.family: Theme.fontMono
                    font.pixelSize: Theme.fsBody
                    color: Theme.ink2
                }

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
                            echoMode: panel.keyVisible ? TextInput.Normal : TextInput.Password
                            selectByMouse: true
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBodyLg
                            color: Theme.ink
                            placeholderText: panel.hasProviderKey
                                             ? qsTr("Введите новый ключ, чтобы заменить сохранённый")
                                             : qsTr("Вставьте API-ключ")
                            placeholderTextColor: Theme.ink4
                            selectionColor: Theme.accentTint
                            selectedTextColor: Theme.ink
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

    // Create the vault ("Защитить паролем") or rotate its passphrase
    // ("Сменить пароль"). `mode` selects which fields show.
    MeetyDialog {
        id: passphraseDialog
        preferredWidth: 420
        property string mode: "create"
        title: mode === "create" ? qsTr("Защитить ключи паролем")
                                  : qsTr("Сменить пароль")
        function openClear() {
            oldField.text = ""
            newField.text = ""
            confirmField.text = ""
            open()
        }
        onAccepted: {
            var cb = function (ok, e) {
                panel.vaultMsg = ok ? "" : e
            }
            if (mode === "create")
                SettingsStore.protectSecretStore(newField.text, cb)
            else
                SettingsStore.changeSecretPassphrase(oldField.text, newField.text, cb)
        }

        MeetyField {
            id: oldField
            Layout.fillWidth: true
            visible: passphraseDialog.mode === "change"
            echoMode: TextInput.Password
            placeholderText: qsTr("Текущий пароль")
        }
        MeetyField {
            id: newField
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: qsTr("Новый пароль")
        }
        MeetyField {
            id: confirmField
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: qsTr("Повторите пароль")
            onAccepted: if (passActions.confirmEnabled) passphraseDialog.accept()
        }
        Text {
            Layout.fillWidth: true
            visible: passphraseDialog.mode === "create"
            text: qsTr("Пароль нельзя восстановить. Если забудете его — сохранённые ключи будут потеряны.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        footer: MeetyDialogActions {
            id: passActions
            dialog: passphraseDialog
            confirmText: passphraseDialog.mode === "create" ? qsTr("Защитить") : qsTr("Сменить")
            confirmVariant: "accent"
            confirmEnabled: newField.text.length > 0
                            && newField.text === confirmField.text
                            && (passphraseDialog.mode !== "change" || oldField.text.length > 0)
        }
    }

    // Forgotten-passphrase escape hatch: wipe the vault and start over.
    MeetyDialog {
        id: resetDialog
        preferredWidth: 420
        title: qsTr("Сбросить хранилище ключей")
        onAccepted: SettingsStore.resetSecretStore(function (ok, e) {
            panel.vaultMsg = ok ? "" : e
        })

        Text {
            Layout.fillWidth: true
            text: qsTr("Сохранённые API-ключи будут безвозвратно удалены — пароль восстановить нельзя. Ключи придётся ввести заново.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink2
        }

        footer: MeetyDialogActions {
            dialog: resetDialog
            confirmText: qsTr("Удалить и сбросить")
            confirmVariant: "accent"
        }
    }
}
