// The three setup steps of the first run, each completed in place: microphone,
// speech recognition model, minutes. Then the way into the first recording.
//
// Nothing here raises a system dialog on its own. The microphone prompt, the
// keychain prompt and the model download each follow a button the user pressed,
// and the row says so before the button is pressed. Loading this pane only
// reads `GET /api/v1/setup-status`, which is silent by contract.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import MeetingAssistant
import "../../i18n/errors.js" as Errors

ColumnLayout {
    id: steps

    // AppPreferences, or null in a context that has none.
    property var preferences: null

    signal startRecording()
    signal importRecording()
    signal skipped()
    // Open the settings at a section: 0 transcription, 1 LLM provider.
    signal openSettings(int category)

    spacing: 12

    readonly property bool known: SetupStore.loaded
    readonly property bool onMac: Qt.platform.os === "osx"

    Component.onCompleted: {
        SetupStore.refresh()
        if (SetupStore.loaded)
            SetupStore.loadCatalog()
        // Which provider is active and where keys are kept. Like the setup
        // status, this read never touches a key.
        if (SettingsStore.loaded)
            steps.provider = SettingsStore.llm().active || "anthropic"
        else
            SettingsStore.refresh()
    }
    Component.onDestruction: stopMonitor()

    Connections {
        target: SetupStore
        function onLoadedChanged() { if (SetupStore.loaded) SetupStore.loadCatalog() }
    }
    Connections {
        target: SettingsStore
        function onLoadedChanged() {
            if (SettingsStore.loaded)
                steps.provider = SettingsStore.llm().active || "anthropic"
        }
    }

    // ── 1. microphone ───────────────────────────────────────────────────────
    // main.cpp's MicrophonePermission; absent where nothing installed it.
    readonly property var permission:
        typeof microphonePermission !== "undefined" ? microphonePermission : null
    readonly property string micPermission: permission ? permission.status : "granted"
    readonly property bool micDenied: micPermission === "denied"
    readonly property bool noMic:
        AudioDevicesStore.loaded && AudioDevicesStore.inputs.length === 0
    // The device list arrives after the test has started: with nothing to
    // listen to, stop rather than wait for a signal that cannot come.
    onNoMicChanged: if (noMic) stopMonitor()

    // "" | "asking" | "testing" | "silent" | "failed"
    property string micPhase: ""
    property bool micHeard: false
    property real micLevel: 0
    property string micError: ""
    property string monitorId: ""

    readonly property bool micBusy:
        micPhase === "asking" || micPhase === "testing" || micPhase === "silent"
    readonly property bool micDone:
        !micDenied && !noMic && (micHeard
                       || (preferences !== null && preferences !== undefined
                           && preferences.microphoneTested === true))

    function testMicrophone() {
        micError = ""
        // Also tells us whether there is a microphone at all.
        AudioDevicesStore.refresh()
        if (permission && micPermission === "undetermined") {
            micPhase = "asking"
            permission.request()
            return
        }
        // The user may have flipped the switch in System Settings meanwhile.
        if (permission)
            permission.refresh()
        if (!micDenied)
            startMonitor()
    }

    function startMonitor() {
        stopMonitor()
        monitorId = "first-run-" + Date.now() + "-" + Math.floor(Math.random() * 1e6)
        micPhase = "testing"
        silenceTimer.restart()
        _monitorStart.post("/api/v1/audio/monitor", { "id": monitorId, "source": "mic" })
    }

    function stopMonitor() {
        pollTimer.stop()
        silenceTimer.stop()
        heardTimer.stop()
        if (monitorId.length)
            _monitorStop.del("/api/v1/audio/monitor/" + monitorId)
        monitorId = ""
        micLevel = 0
        if (micPhase !== "failed")
            micPhase = ""
    }

    Connections {
        target: steps.permission
        enabled: steps.permission !== null
        function onStatusChanged() {
            if (steps.micPhase !== "asking")
                return
            if (steps.micPermission === "granted")
                steps.startMonitor()
            else
                steps.micPhase = ""
        }
    }

    Request {
        id: _monitorStart
        onOk: function (json) { pollTimer.start() }
        onFail: function (s, e, body) {
            steps.monitorId = ""
            steps.micError = e
            steps.micPhase = "failed"
            silenceTimer.stop()
        }
    }
    Request {
        id: _monitorPoll
        onOk: function (json) {
            steps.micLevel = (json && json.level) || 0
            if (steps.micLevel > 0.02 && !steps.micHeard) {
                steps.micHeard = true
                if (steps.preferences && steps.preferences.markMicrophoneTested)
                    steps.preferences.markMicrophoneTested()
                // Let the user watch the meter move for a moment, then let go
                // of the microphone.
                heardTimer.restart()
            }
        }
        onFail: function (s, e) {}
    }
    Request {
        id: _monitorStop
        onOk: function (json) {}
        onFail: function (s, e) {}
    }
    Timer {
        id: pollTimer
        interval: 90
        repeat: true
        onTriggered: if (steps.monitorId.length && !_monitorPoll.busy)
                         _monitorPoll.get("/api/v1/audio/monitor/" + steps.monitorId)
    }
    Timer {
        id: silenceTimer
        interval: 8000
        onTriggered: if (!steps.micHeard && steps.micPhase === "testing")
                         steps.micPhase = "silent"
    }
    Timer {
        id: heardTimer
        interval: 2500
        onTriggered: steps.stopMonitor()
    }


    // ── 2. speech recognition model ─────────────────────────────────────────
    readonly property var transcription: SetupStore.snapshot.transcription || ({})
    readonly property string recommendedId: transcription.recommended_model_id || ""
    // "unknown" | "cpu" | "ready" | "downloading" | "failed" | "todo"
    readonly property string modelState:
        !known ? "unknown"
        : !SetupStore.cpuSupported ? "cpu"
        : SetupStore.transcriptionState === "ready" ? "ready"
        : (SetupStore.installing || SetupStore.transcriptionState === "downloading") ? "downloading"
        : SetupStore.installError.length > 0 ? "failed"
        : "todo"
    readonly property int modelPercent:
        SetupStore.installing ? SetupStore.installPercent : SetupStore.downloadPercent


    // ── 3. minutes ──────────────────────────────────────────────────────────
    readonly property var providers: [
        { id: "anthropic", label: "Anthropic" },
        { id: "openai", label: "OpenAI" },
        { id: "gemini", label: "Gemini" },
        { id: "mistral", label: "Mistral" },
        { id: "ollama", label: qsTr("Ollama (на этом компьютере)") }
    ]
    property string provider: "anthropic"
    readonly property bool keyless: provider === "ollama"
    // "" | "checking" | "failed"
    property string minutesPhase: ""
    property string minutesError: ""

    readonly property bool minutesSkipped: known && SetupStore.minutesState === "skipped"
    readonly property bool minutesLocked: known && SetupStore.minutesState === "locked"
    readonly property bool minutesDone:
        known && SetupStore.minutesState === "ready" && minutesPhase === ""

    function providerLabel(id) {
        for (var i = 0; i < providers.length; ++i)
            if (providers[i].id === id)
                return id === "ollama" ? "Ollama" : providers[i].label
        return id
    }

    function minutesFailed(errorClass, message) {
        if (keyless && errorClass === "network_timeout") {
            minutesError = qsTr("Ollama не отвечает. Запустите её и повторите проверку.")
        } else if (keyless && errorClass === "llm_model_unavailable") {
            var model = SettingsStore.providerCfg("ollama").model
            minutesError = qsTr("В Ollama нет модели %1. Скачайте её командой «ollama pull %1» и повторите проверку.").arg(model)
        } else if (errorClass === "api_auth") {
            // The shared copy sends the user to the settings; here the key
            // field is right under this line.
            minutesError = qsTr("Провайдер не принял ключ. Проверьте его и попробуйте ещё раз.")
        } else {
            var d = Errors.describe(errorClass, message)
            minutesError = d.detail ? d.hint + " " + d.detail : d.hint
        }
        minutesPhase = "failed"
    }

    // Save the key if one was typed, make the provider the active one, then ask
    // it something small. The step is done only when that answer comes back.
    function checkMinutes() {
        var chosen = provider
        var key = keyField.text.trim()
        minutesError = ""
        if (!keyless && key.length === 0 && SettingsStore.providerCfg(chosen).has_key !== true) {
            minutesError = qsTr("Введите API-ключ.")
            minutesPhase = "failed"
            return
        }
        minutesPhase = "checking"
        var probe = function () {
            SetupStore.setActiveProvider(chosen, function (ok, error) {
                if (!ok) {
                    steps.minutesFailed("", error)
                    return
                }
                SettingsStore.testProvider(chosen, "", function (passed, errorClass, message) {
                    if (passed) {
                        keyField.text = ""
                        steps.minutesPhase = ""
                        SetupStore.refresh()
                    } else {
                        steps.minutesFailed(errorClass, message)
                    }
                })
            })
        }
        if (!keyless && key.length > 0) {
            SettingsStore.setSecret(chosen, key, function (ok, error) {
                if (ok)
                    probe()
                else
                    steps.minutesFailed("", error)
            })
        } else {
            probe()
        }
    }


    // The steps scroll; what follows them stays pinned below, so the way into
    // the first recording is in view however tall the steps get.
    ScrollView {
        id: stepsScroll
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumHeight: 120
        implicitHeight: stepsColumn.implicitHeight
        contentWidth: availableWidth
        clip: true
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

        ColumnLayout {
            id: stepsColumn
            width: stepsScroll.availableWidth
            spacing: 12

            SetupStepRow {
                objectName: "micStep"
                Layout.fillWidth: true
                number: "1"
                title: qsTr("Микрофон")
                status: steps.noMic ? "attention"
                      : steps.micDone && !steps.micBusy ? "done"
                      : steps.micBusy ? "busy"
                      : (steps.micDenied || steps.micPhase === "failed") ? "attention"
                      : "todo"
                summary: steps.noMic ? qsTr("Микрофон не найден. Подключите его или импортируйте готовую запись.")
                       : steps.micDone && !steps.micBusy ? qsTr("Микрофон слышно.")
                       : steps.micDenied ? qsTr("Доступ к микрофону запрещён. Разрешите его в системных настройках и нажмите «Проверить снова».")
                       : steps.micPhase === "failed" ? qsTr("Не удалось открыть микрофон: %1").arg(steps.micError)
                       : steps.micPhase === "asking" ? qsTr("Ответьте на запрос системы.")
                       : steps.micPhase === "silent" ? qsTr("Сигнала пока нет. Проверьте, что микрофон не выключен, и скажите что-нибудь.")
                       : steps.micPhase === "testing" ? (steps.micHeard ? qsTr("Микрофон слышно.") : qsTr("Скажите что-нибудь…"))
                       : steps.onMac ? qsTr("macOS спросит разрешение на доступ к микрофону.")
                       : qsTr("Скажите пару слов, чтобы убедиться, что вас слышно.")

                // Button groups are Flows, not RowLayouts: a row that cannot shrink
                // would set the minimum width of the whole column and push every
                // text in it past the edge of a narrow pane.
                Flow {
                    Layout.fillWidth: true
                    spacing: 8

                    MeetyButton {
                        objectName: "micTestButton"
                        text: steps.micBusy ? qsTr("Остановить")
                            : (steps.micDenied || steps.micPhase === "failed") ? qsTr("Проверить снова")
                            : qsTr("Проверить микрофон")
                        iconName: "mic"
                        onClicked: steps.micBusy ? steps.stopMonitor() : steps.testMicrophone()
                    }
                    MeetyButton {
                        visible: steps.micDenied && steps.onMac
                        variant: "ghost"
                        text: qsTr("Открыть настройки macOS")
                        onClicked: Qt.openUrlExternally(
                            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
                    }
                }
                MeetyLevelMeter {
                    Layout.fillWidth: true
                    Layout.maximumWidth: 220
                    visible: steps.micPhase === "testing" || steps.micPhase === "silent"
                    level: steps.micLevel
                    active: true
                }
            }

            SetupStepRow {
                objectName: "modelStep"
                Layout.fillWidth: true
                number: "2"
                title: qsTr("Распознавание речи")
                status: steps.modelState === "ready" ? "done"
                      : steps.modelState === "downloading" ? "busy"
                      : (steps.modelState === "failed" || steps.modelState === "cpu") ? "attention"
                      : "todo"
                summary: steps.modelState === "ready"
                           ? (steps.transcription.model_id
                              ? qsTr("Модель установлена: %1.").arg(steps.transcription.model_id)
                              : qsTr("Модель установлена."))
                       : steps.modelState === "cpu" ? qsTr("На этом компьютере расшифровка не заработает: процессор не поддерживает AVX2.")
                       : steps.modelState === "downloading"
                           ? (steps.modelPercent >= 0
                              ? qsTr("Модель скачивается: %1%. Остальное можно делать, не дожидаясь.").arg(steps.modelPercent)
                              : qsTr("Модель скачивается. Остальное можно делать, не дожидаясь."))
                       : steps.modelState === "failed" ? qsTr("Не удалось скачать модель: %1").arg(SetupStore.installError)
                       : SetupStore.recommendedModel
                           ? qsTr("Работает на вашем компьютере. Нужна модель: %1, %2.")
                                 .arg(SetupStore.recommendedModel.display_name)
                                 .arg(SetupStore.recommendedModel.approximate_size)
                       : qsTr("Работает на вашем компьютере. Нужна модель.")

                Rectangle {
                    Layout.fillWidth: true
                    Layout.maximumWidth: 320
                    Layout.preferredHeight: 6
                    visible: steps.modelState === "downloading"
                    radius: 3
                    color: Theme.rule

                    Rectangle {
                        height: parent.height
                        radius: 3
                        width: parent.width * Math.max(0, Math.min(100, steps.modelPercent)) / 100
                        color: Theme.accent
                        Behavior on width { NumberAnimation { duration: 200 } }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    visible: steps.modelState === "todo" || steps.modelState === "failed"
                    spacing: 8

                    MeetyButton {
                        objectName: "modelDownloadButton"
                        enabled: steps.recommendedId.length > 0
                        iconName: "download"
                        text: steps.modelState === "failed" ? qsTr("Повторить") : qsTr("Скачать модель")
                        onClicked: SetupStore.installModel(steps.recommendedId)
                    }
                    MeetyButton {
                        variant: "ghost"
                        text: qsTr("Другая модель")
                        onClicked: steps.openSettings(0)
                    }
                }
            }

            SetupStepRow {
                objectName: "minutesStep"
                Layout.fillWidth: true
                number: "3"
                title: qsTr("Протоколы")
                status: (steps.minutesDone || steps.minutesSkipped) ? "done"
                      : steps.minutesPhase === "checking" ? "busy"
                      : (steps.minutesPhase === "failed" || steps.minutesLocked) ? "attention"
                      : "todo"
                summary: steps.minutesDone
                           ? qsTr("Протоколы будет писать %1.").arg(steps.providerLabel(SettingsStore.llm().active || steps.provider))
                       : steps.minutesSkipped ? qsTr("Настроите позже: «Настройки» → «LLM-провайдер».")
                       : steps.minutesPhase === "checking" ? qsTr("Проверяем…")
                       : steps.minutesPhase === "failed" ? steps.minutesError
                       : steps.minutesLocked ? qsTr("Хранилище ключей заблокировано. Разблокируйте его в настройках.")
                       : qsTr("Выберите ИИ для протоколов. Расшифровка уходит ему только при генерации.")

                MeetyComboBox {
                    id: providerBox
                    objectName: "providerBox"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 320
                    visible: !steps.minutesLocked
                    textRole: "label"
                    valueRole: "id"
                    model: steps.providers
                    currentIndex: {
                        for (var i = 0; i < steps.providers.length; ++i)
                            if (steps.providers[i].id === steps.provider)
                                return i
                        return 0
                    }
                    onActivated: {
                        steps.provider = currentValue
                        steps.minutesError = ""
                        if (steps.minutesPhase === "failed")
                            steps.minutesPhase = ""
                    }
                }
                MeetyField {
                    id: keyField
                    objectName: "keyField"
                    Layout.fillWidth: true
                    Layout.maximumWidth: 320
                    visible: !steps.keyless && !steps.minutesLocked
                    echoMode: TextInput.Password
                    placeholderText: SettingsStore.providerCfg(steps.provider).has_key === true
                                     ? qsTr("Ключ уже сохранён")
                                     : qsTr("API-ключ")
                    onAccepted: steps.checkMinutes()
                }
                Text {
                    Layout.fillWidth: true
                    visible: !steps.minutesLocked
                             && (steps.keyless
                                 || (steps.onMac && SettingsStore.secretStorage().kind === "keyring"))
                    text: steps.keyless
                          ? qsTr("Ключ не нужен, расшифровка не покидает компьютер.")
                          : qsTr("macOS может спросить пароль от связки ключей.")
                    wrapMode: Text.WordWrap
                    lineHeight: 1.4
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8

                    MeetyButton {
                        objectName: "minutesCheckButton"
                        visible: !steps.minutesLocked
                        enabled: steps.minutesPhase !== "checking"
                        text: qsTr("Проверить")
                        onClicked: steps.checkMinutes()
                    }
                    MeetyButton {
                        visible: steps.minutesLocked
                        text: qsTr("Открыть настройки")
                        onClicked: steps.openSettings(1)
                    }
                    MeetyButton {
                        objectName: "minutesLaterButton"
                        variant: "ghost"
                        enabled: steps.minutesPhase !== "checking"
                        text: qsTr("Позже")
                        onClicked: SetupStore.skipMinutes()
                    }
                }
            }
        }
    }

    // ── what the user should know, then the way in ──────────────────────────
    Text {
        Layout.fillWidth: true
        Layout.topMargin: 4
        text: qsTr("Звук не покидает компьютер. Предупредите участников о записи.")
        wrapMode: Text.WordWrap
        lineHeight: 1.4
        font.family: Theme.fontUi
        font.pixelSize: Theme.fsSmall
        color: Theme.ink3
    }

    MeetyButton {
        objectName: "startButton"
        Layout.alignment: Qt.AlignLeft
        enabled: steps.micDone
        text: qsTr("Начать первую запись")
        iconName: "mic"
        variant: "accent"
        large: true
        onClicked: steps.startRecording()
    }
    Text {
        objectName: "startCaption"
        Layout.fillWidth: true
        Layout.topMargin: -8
        visible: text.length > 0
        text: !steps.micDone ? qsTr("Сначала проверьте микрофон.")
            : (steps.modelState === "todo" || steps.modelState === "failed" || steps.modelState === "downloading")
              ? qsTr("Расшифровать запись получится, когда модель будет установлена.")
            : ""
        wrapMode: Text.WordWrap
        lineHeight: 1.4
        font.family: Theme.fontUi
        font.pixelSize: Theme.fsSmall
        color: Theme.ink3
    }

    Flow {
        Layout.fillWidth: true
        Layout.topMargin: -4
        // Ghost buttons carry their own side padding: pull the first one back
        // so its text lines up with the column.
        Layout.leftMargin: -13
        spacing: 0

        MeetyButton {
            variant: "ghost"
            text: qsTr("Импорт записи")
            onClicked: steps.importRecording()
        }
        MeetyButton {
            objectName: "skipButton"
            variant: "ghost"
            text: qsTr("Пропустить настройку")
            onClicked: steps.skipped()
        }
    }
}
