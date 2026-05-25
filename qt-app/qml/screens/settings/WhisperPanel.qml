// Whisper transcriber settings. Edits scr.draft.transcriber and model paths.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import Qt.labs.folderlistmodel
import MeetingAssistant

ScrollView {
    id: panel
    property var scr
    property var modelState: ({ models: [], models_dir: "" })
    property string modelError: ""
    property bool addCustomOpen: false
    property string addPath: ""
    property string addName: ""
    property string addDescription: ""
    property string pendingDeleteModelId: ""
    property string pendingDeleteModelName: ""
    property string modelsDirDraft: ""
    property url modelsDirBrowserUrl: ""

    font.family: Theme.fontUi
    font.pixelSize: Theme.fsBody
    clip: true
    contentWidth: availableWidth

    function t() { return scr.draft.transcriber || (scr.draft.transcriber = {}) }
    function p() { return scr.draft.paths || (scr.draft.paths = {}) }
    function modelsDir() { return p().models_dir || modelState.models_dir || "" }
    function customModels() { return t().custom_models || (t().custom_models = []) }
    function refreshModels() {
        modelError = ""
        modelsReq.get("/api/v1/transcription-models")
    }
    function load() {
        var tr = t()
        langBox.setValue(tr.language || "ru")
        beamSpin.value = tr.beam_size || 1
        threadsSpin.value = tr.n_threads || 0
        refreshModels()
    }
    function catalogPath(model) {
        var dir = modelsDir()
        if (!dir || !model || !model.filename)
            return ""
        return dir.replace(/\/$/, "") + "/" + model.filename
    }
    function fileUrl(path) {
        if (!path || path.length === 0)
            return ""
        return path.charAt(0) === "/" ? "file://" + path : "file:///" + path
    }
    function filename(path) {
        var parts = (path || "").split("/")
        return parts.length > 0 ? parts[parts.length - 1] : ""
    }
    function parentDir(path) {
        var clean = (path || "").replace(/\/+$/, "")
        var idx = clean.lastIndexOf("/")
        if (idx <= 0)
            return idx === 0 ? "/" : ""
        return clean.slice(0, idx)
    }
    function openActiveModelFolder() {
        var dir = parentDir(activePath())
        if (dir.length === 0)
            return
        if (!Qt.openUrlExternally(fileUrl(dir)))
            modelError = qsTr("Не удалось открыть папку модели: %1").arg(dir)
    }
    function nameFromPath(path) {
        var name = filename(path)
        name = name.replace(/^ggml-/, "")
        name = name.replace(/\.bin$/, "")
        return name.length > 0 ? name : qsTr("Своя модель")
    }
    function findCatalog(id) {
        var models = modelState.models || []
        for (var i = 0; i < models.length; i++) {
            if (models[i].id === id)
                return models[i]
        }
        return null
    }
    function findCustomByPath(path) {
        var models = customModels()
        for (var i = 0; i < models.length; i++) {
            if (models[i].path === path)
                return models[i]
        }
        return null
    }
    function activeModel() {
        scr.rev
        var tr = t()
        if (tr.model_source === "managed" && tr.model_id)
            return findCatalog(tr.model_id)
        if (tr.custom_model_path) {
            var custom = findCustomByPath(tr.custom_model_path)
            if (custom)
                return custom
            return {
                id: "custom-active",
                name: nameFromPath(tr.custom_model_path),
                path: tr.custom_model_path,
                description: qsTr("Внешний ggml-совместимый файл. Будет использоваться для новых транскрипций.")
            }
        }
        return null
    }
    function activePath() {
        var model = activeModel()
        if (!model)
            return ""
        return model.path || catalogPath(model)
    }
    function activeDescription() {
        var model = activeModel()
        if (!model)
            return qsTr("Выберите установленную модель из каталога или добавьте внешний .bin файл.")
        return model.description || qsTr("Внешний ggml-совместимый файл. Будет использоваться для новых транскрипций.")
    }
    function activeName() {
        var model = activeModel()
        return model ? (model.display_name || model.name || model.id) : qsTr("Модель не выбрана")
    }
    function activeSize() {
        var model = activeModel()
        return model ? (model.approximate_size || model.size || "") : ""
    }
    function isManagedActive(id) {
        scr.rev
        return t().model_source === "managed" && t().model_id === id
    }
    function isCustomActive(path) {
        scr.rev
        return t().model_source === "custom_path" && t().custom_model_path === path
    }
    function installedModels() {
        scr.rev
        var rows = []
        var models = modelState.models || []
        for (var i = 0; i < models.length; i++) {
            if (models[i].installed)
                rows.push(models[i])
        }
        rows.sort(function(a, b) {
            if (isManagedActive(a.id)) return -1
            if (isManagedActive(b.id)) return 1
            return (a.display_name || a.id).localeCompare(b.display_name || b.id)
        })
        return rows
    }
    function availableModels() {
        var rows = []
        var models = modelState.models || []
        for (var i = 0; i < models.length; i++) {
            if (!models[i].installed)
                rows.push(models[i])
        }
        return rows
    }
    function selectManaged(modelId) {
        var tr = t()
        tr.model_source = "managed"
        tr.model_id = modelId
        scr.touch()
    }
    function selectCustom(path) {
        var tr = t()
        tr.model_source = "custom_path"
        tr.custom_model_path = path
        tr.model_path = path
        scr.touch()
    }
    function addCustomModel() {
        var path = addPath.trim()
        if (!path.endsWith(".bin"))
            return
        var model = {
            id: "custom-" + Date.now(),
            name: addName.trim() || nameFromPath(path),
            path: path,
            description: addDescription.trim().length > 0 ? addDescription.trim() : null
        }
        customModels().push(model)
        addCustomOpen = false
        addPath = ""
        addName = ""
        addDescription = ""
        scr.touch()
    }
    function removeCustom(path) {
        var models = customModels()
        for (var i = models.length - 1; i >= 0; i--) {
            if (models[i].path === path)
                models.splice(i, 1)
        }
        scr.touch()
    }
    function requestDeleteManaged(model) {
        pendingDeleteModelId = model.id
        pendingDeleteModelName = model.display_name || model.id
        deleteManagedDialog.open()
    }
    function pathFromUrl(url) {
        return decodeURIComponent(url.toString().replace(/^file:\/\//, ""))
    }
    function openModelsDirDialog() {
        modelsDirDraft = modelsDir()
        modelsDirDialog.open()
    }
    function openModelsDirPicker() {
        var dir = modelsDirDraft.trim().length > 0 ? modelsDirDraft.trim() : modelsDir()
        if (dir.length > 0)
            modelsDirBrowserUrl = fileUrl(dir)
        else
            modelsDirBrowserUrl = fileUrl("/")
        modelsDirBrowserDialog.open()
    }
    function applyModelsDir(path) {
        var dir = path.trim()
        if (dir.length === 0)
            return
        p().models_dir = dir
        scr.touch()
        refreshModels()
    }

    Component.onCompleted: load()
    Connections { target: scr; function onReseeded() { panel.load() } }

    property Request modelsReq: Request {
        onOk: function(json) { panel.modelState = json || ({ models: [], models_dir: "" }) }
        onFail: function(s, e) {
            panel.modelError = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
        }
    }
    property Request deleteManagedReq: Request {
        onOk: function(json) { panel.refreshModels() }
        onFail: function(s, e) {
            panel.modelError = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
        }
    }

    FileDialog {
        id: customModelPicker
        title: qsTr("Выберите файл модели Whisper")
        nameFilters: [qsTr("Модель Whisper (*.bin)"), qsTr("Все файлы (*)")]
        onAccepted: {
            panel.addPath = selectedFile.toString().replace(/^file:\/\//, "")
        }
    }

    MeetyDialog {
        id: deleteManagedDialog
        title: qsTr("Удалить модель?")
        preferredWidth: 430
        Text {
            Layout.fillWidth: true
            text: qsTr("Файл модели «%1» будет удалён с диска. Это действие нельзя отменить.").arg(panel.pendingDeleteModelName)
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBodyLg
            color: Theme.ink2
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 10
            spacing: 8
            Item { Layout.fillWidth: true }
            MeetyButton {
                variant: "ghost"
                text: qsTr("Отмена")
                onClicked: deleteManagedDialog.reject()
            }
            MeetyButton {
                variant: "accent"
                iconName: "trash"
                text: qsTr("Удалить")
                onClicked: {
                    deleteManagedReq.del("/api/v1/transcription-models/" + panel.pendingDeleteModelId)
                    deleteManagedDialog.accept()
                }
            }
        }
    }

    MeetyDialog {
        id: modelsDirDialog
        title: qsTr("Папка моделей")
        preferredWidth: 560
        onAccepted: panel.applyModelsDir(panel.modelsDirDraft)

        Text {
            Layout.fillWidth: true
            text: qsTr("Можно ввести путь вручную. Это удобно для скрытых папок вроде ~/.local.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 8
            MeetyField {
                id: modelsDirInput
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: panel.modelsDirDraft
                placeholderText: "/home/dmitry/.local/share/meeting-assistant/models"
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                onTextChanged: panel.modelsDirDraft = text
                onAccepted: {
                    if (panel.modelsDirDraft.trim().length > 0)
                        modelsDirDialog.accept()
                }
            }
            MeetyButton {
                variant: "ghost"
                iconName: "folder"
                text: qsTr("Обзор")
                onClicked: panel.openModelsDirPicker()
            }
        }

        footer: MeetyDialogActions {
            dialog: modelsDirDialog
            cancelText: qsTr("Отмена")
            confirmText: qsTr("Применить")
            confirmVariant: "accent"
            confirmEnabled: panel.modelsDirDraft.trim().length > 0
        }
    }

    MeetyDialog {
        id: modelsDirBrowserDialog
        title: qsTr("Выбор папки моделей")
        preferredWidth: 620

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            MeetyButton {
                variant: "ghost"
                iconName: "arrow-left"
                text: qsTr("Назад")
                enabled: modelsDirFolderModel.parentFolder.toString().length > 0
                onClicked: panel.modelsDirBrowserUrl = modelsDirFolderModel.parentFolder
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                implicitHeight: 34
                radius: Theme.rMd
                color: Theme.paperSub
                border.width: 1
                border.color: Theme.rule

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 10
                    anchors.rightMargin: 10
                    spacing: 7
                    MeetyIcon {
                        name: "folder"
                        size: 13
                        color: Theme.ink3
                    }
                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: panel.pathFromUrl(panel.modelsDirBrowserUrl)
                        elide: Text.ElideLeft
                        font.family: Theme.fontMono
                        font.pixelSize: Theme.fsSmall
                        color: Theme.ink3
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 360
            radius: Theme.rMd
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule
            clip: true

            ListView {
                id: modelsDirList
                anchors.fill: parent
                anchors.margins: 3
                clip: true
                model: FolderListModel {
                    id: modelsDirFolderModel
                    folder: panel.modelsDirBrowserUrl
                    showFiles: false
                    showDirs: true
                    showDirsFirst: true
                    showDotAndDotDot: false
                    showHidden: true
                    sortField: FolderListModel.Name
                    sortCaseSensitive: false
                }
                delegate: Rectangle {
                    id: folderRow
                    required property string fileName
                    required property url fileUrl
                    required property int index
                    width: ListView.view.width
                    height: 34
                    radius: Theme.rSm
                    color: folderHover.containsMouse || ListView.isCurrentItem ? Theme.paper : "transparent"

                    RowLayout {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 8
                        MeetyIcon {
                            name: "folder"
                            size: 14
                            color: Theme.ink3
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: folderRow.fileName
                            elide: Text.ElideRight
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBody
                            color: Theme.ink2
                        }
                    }

                    MouseArea {
                        id: folderHover
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: modelsDirList.currentIndex = folderRow.index
                        onDoubleClicked: panel.modelsDirBrowserUrl = folderRow.fileUrl
                    }
                }
            }
        }

        footer: Item {
            implicitHeight: browserActions.implicitHeight + 20
            RowLayout {
                id: browserActions
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.rightMargin: 20
                spacing: 8
                MeetyButton {
                    variant: "ghost"
                    text: qsTr("Отмена")
                    onClicked: modelsDirBrowserDialog.reject()
                }
                MeetyButton {
                    variant: "accent"
                    text: qsTr("Выбрать эту папку")
                    onClicked: {
                        panel.modelsDirDraft = panel.pathFromUrl(panel.modelsDirBrowserUrl)
                        modelsDirBrowserDialog.accept()
                    }
                }
            }
        }
    }

    ColumnLayout {
        width: panel.availableWidth
        spacing: 0

        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.topMargin: 32
            text: qsTr("Транскрипция")
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
            Layout.bottomMargin: 24
            text: qsTr("Whisper работает локально на вашем устройстве. Аудио никогда не покидает компьютер.")
            wrapMode: Text.WordWrap
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsBody
            color: Theme.ink3
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.bottomMargin: 28
            implicitHeight: heroRow.implicitHeight + 36
            radius: Theme.rLg
            color: Theme.paperSub
            border.width: 1
            border.color: Theme.rule

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.topMargin: 14
                anchors.bottomMargin: 14
                width: 3
                radius: 2
                color: Theme.accent
            }

            RowLayout {
                id: heroRow
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: 20
                anchors.rightMargin: 20
                spacing: 16

                Rectangle {
                    Layout.preferredWidth: 44
                    Layout.preferredHeight: 44
                    Layout.alignment: Qt.AlignTop
                    radius: 10
                    color: Theme.accentTint
                    MeetyIcon {
                        anchors.centerIn: parent
                        name: "waveform"
                        size: 20
                        color: Theme.accent2
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text {
                        Layout.fillWidth: true
                        text: qsTr("АКТИВНАЯ МОДЕЛЬ")
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsMicro
                        font.weight: Theme.wSemiBold
                        font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.14)
                        color: Theme.ink3
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Text {
                            Layout.fillWidth: true
                            text: panel.activeName()
                            elide: Text.ElideRight
                            font.family: Theme.fontSerif
                            font.pixelSize: Theme.fsTitle
                            font.weight: Theme.wMedium
                            font.letterSpacing: 0
                            color: Theme.ink
                        }
                        MeetyTag {
                            visible: panel.activeSize().length > 0
                            mono: true
                            text: panel.activeSize()
                            color: Theme.paper3
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: panel.activeDescription()
                        wrapMode: Text.WordWrap
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBody
                        color: Theme.ink2
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        visible: panel.activePath().length > 0
                        spacing: 6
                        MeetyIcon {
                            name: "folder"
                            size: 11
                            color: Theme.ink3
                        }
                        Text {
                            Layout.fillWidth: true
                            text: panel.activePath()
                            elide: Text.ElideLeft
                            font.family: Theme.fontMono
                            font.pixelSize: Theme.fsSmall
                            color: Theme.ink3
                        }
                    }
                }

                MeetyButton {
                    variant: "ghost"
                    iconName: "folder"
                    text: qsTr("Показать")
                    enabled: panel.activePath().length > 0
                    onClicked: panel.openActiveModelFolder()
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.bottomMargin: 22
            spacing: 12

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        spacing: 3
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: qsTr("Каталог моделей")
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBodyLg
                            font.weight: Theme.wSemiBold
                            color: Theme.ink
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            text: qsTr("Whisper.cpp модели из выбранной папки. Установленные модели можно выбрать кликом по строке.")
                            wrapMode: Text.WordWrap
                            font.family: Theme.fontUi
                            font.pixelSize: Theme.fsBody
                            color: Theme.ink3
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    FolderChip {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        folderPath: panel.modelsDir().length > 0 ? panel.modelsDir() : qsTr("Папка моделей")
                        onClicked: panel.openModelsDirDialog()
                    }
                    MeetyIconButton {
                        iconName: "refresh"
                        enabled: !modelsReq.busy
                        onClicked: panel.refreshModels()
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                visible: panel.modelError.length > 0
                text: panel.modelError
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.rec
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: catalogList.implicitHeight + 6
                radius: Theme.rMd
                color: Theme.paperSub
                border.width: 1
                border.color: Theme.rule

                ColumnLayout {
                    id: catalogList
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 3
                    spacing: 1

                    ModelSubLabel {
                        Layout.fillWidth: true
                        label: qsTr("На устройстве")
                        count: panel.installedModels().length
                    }

                    Repeater {
                        model: panel.installedModels()
                        delegate: InstalledCatalogRow {
                            required property var modelData
                            Layout.fillWidth: true
                            modelInfo: modelData
                            active: panel.isManagedActive(modelData.id)
                            onSelect: panel.selectManaged(modelData.id)
                            onDeleteRequested: panel.requestDeleteManaged(modelData)
                        }
                    }

                    ModelSubLabel {
                        Layout.fillWidth: true
                        topDivider: panel.installedModels().length > 0
                        label: qsTr("Доступно для загрузки")
                        count: panel.availableModels().length
                    }

                    Repeater {
                        model: panel.availableModels()
                        delegate: AvailableCatalogRow {
                            required property var modelData
                            Layout.fillWidth: true
                            modelInfo: modelData
                        }
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 32
            Layout.bottomMargin: 22
            spacing: 10
            Text {
                Layout.fillWidth: true
                text: qsTr("Модели вне каталога")
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBodyLg
                font.weight: Theme.wSemiBold
                color: Theme.ink
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("Внешние ggml-*.bin файлы. Файлы остаются на своих местах.")
                wrapMode: Text.WordWrap
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsBody
                color: Theme.ink3
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: customList.implicitHeight + 6
                radius: Theme.rMd
                color: Theme.paperSub
                border.width: 1
                border.color: Theme.rule

                ColumnLayout {
                    id: customList
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 3
                    spacing: 1

                    Repeater {
                        model: panel.customModels()
                        delegate: CustomModelRow {
                            required property var modelData
                            Layout.fillWidth: true
                            modelInfo: modelData
                            active: panel.isCustomActive(modelData.path)
                            onSelect: panel.selectCustom(modelData.path)
                            onRemove: panel.removeCustom(modelData.path)
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        visible: !panel.addCustomOpen
                        implicitHeight: addRowContent.implicitHeight + 24
                        radius: Theme.rSm
                        color: addHover.containsMouse ? Theme.paper : "transparent"
                        border.width: 1
                        border.color: addHover.containsMouse ? Theme.accent : Theme.rule2
                        border.pixelAligned: true

                        RowLayout {
                            id: addRowContent
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: 14
                            anchors.rightMargin: 14
                            spacing: 10
                            MeetyIcon { name: "plus"; size: 14; color: Theme.ink3 }
                            Text {
                                Layout.fillWidth: true
                                Layout.minimumWidth: 0
                                text: qsTr("<b>Добавить файл модели</b> — ggml-совместимый .bin с диска")
                                textFormat: Text.RichText
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsBody
                                color: Theme.ink3
                            }
                        }
                        MouseArea {
                            id: addHover
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: panel.addCustomOpen = true
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        visible: panel.addCustomOpen
                        implicitHeight: addForm.implicitHeight + 28
                        radius: Theme.rSm
                        color: Theme.paper
                        border.width: 1
                        border.color: Theme.rule

                        Rectangle {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            anchors.topMargin: 14
                            anchors.bottomMargin: 14
                            width: 2
                            radius: 2
                            color: Theme.accent
                        }

                        ColumnLayout {
                            id: addForm
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: 18
                            spacing: 12

                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Добавить свой файл модели")
                                font.family: Theme.fontSerif
                                font.pixelSize: 16
                                font.weight: Theme.wMedium
                                font.letterSpacing: 0
                                color: Theme.ink
                            }
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Whisper.cpp ggml-*.bin или совместимый. Meety только запоминает путь.")
                                wrapMode: Text.WordWrap
                                font.family: Theme.fontUi
                                font.pixelSize: Theme.fsBody
                                color: Theme.ink3
                            }

                            AddFieldRow {
                                label: qsTr("Файл")
                                requiredText: true
                                MeetyField {
                                    Layout.fillWidth: true
                                    text: panel.addPath
                                    placeholderText: "/path/to/ggml-large-v3.bin"
                                    font.family: Theme.fontMono
                                    font.pixelSize: Theme.fsSmall
                                    onTextChanged: panel.addPath = text
                                }
                                MeetyButton {
                                    variant: "ghost"
                                    iconName: "folder"
                                    text: qsTr("Обзор")
                                    onClicked: customModelPicker.open()
                                }
                            }

                            AddFieldRow {
                                label: qsTr("Название")
                                optional: true
                                MeetyField {
                                    Layout.fillWidth: true
                                    text: panel.addName
                                    placeholderText: panel.addPath.length > 0 ? panel.nameFromPath(panel.addPath) : qsTr("Whisper FT")
                                    onTextChanged: panel.addName = text
                                }
                            }

                            AddFieldRow {
                                label: qsTr("Описание")
                                optional: true
                                TextArea {
                                    Layout.fillWidth: true
                                    text: panel.addDescription
                                    placeholderText: qsTr("Например: дообученная под технические встречи на русском.")
                                    wrapMode: TextArea.Wrap
                                    font.family: Theme.fontUi
                                    font.pixelSize: Theme.fsBodyLg
                                    color: Theme.ink
                                    placeholderTextColor: Theme.ink4
                                    onTextChanged: panel.addDescription = text
                                    background: Rectangle {
                                        implicitHeight: 58
                                        radius: Theme.rMd
                                        color: Theme.paper
                                        border.width: 1
                                        border.color: parent.activeFocus ? Theme.focus : Theme.rule
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8
                                Item { Layout.fillWidth: true }
                                MeetyButton {
                                    variant: "ghost"
                                    text: qsTr("Отмена")
                                    onClicked: {
                                        panel.addCustomOpen = false
                                        panel.addPath = ""
                                        panel.addName = ""
                                        panel.addDescription = ""
                                    }
                                }
                                MeetyButton {
                                    variant: "primary"
                                    text: qsTr("Добавить")
                                    enabled: panel.addPath.trim().endsWith(".bin")
                                    onClicked: panel.addCustomModel()
                                }
                            }
                        }
                    }
                }
            }
        }

        SettingsRow {
            title: qsTr("Язык")
            help: qsTr("«Автоопределение» выберет язык по содержимому.")
            MeetyComboBox {
                id: langBox
                Layout.fillWidth: true
                textRole: "label"
                valueRole: "code"
                model: [
                    { code: "auto", label: qsTr("Автоопределение") },
                    { code: "ru",   label: qsTr("Русский") },
                    { code: "en",   label: qsTr("Английский") },
                    { code: "de",   label: qsTr("Немецкий") },
                    { code: "fr",   label: qsTr("Французский") },
                    { code: "es",   label: qsTr("Испанский") }
                ]
                function setValue(code) {
                    var i = indexOfValue(code)
                    currentIndex = i >= 0 ? i : 1
                }
                onActivated: { panel.t().language = currentValue; scr.touch() }
            }
        }

        SettingsRow {
            title: qsTr("Beam size")
            help: qsTr("Больше — потенциально точнее, но медленнее.")
            MeetySpinBox {
                id: beamSpin
                Layout.fillWidth: true
                from: 1; to: 8
                onValueModified: { panel.t().beam_size = value; scr.touch() }
            }
        }

        SettingsRow {
            title: qsTr("Потоки CPU")
            help: qsTr("0 — автоматический выбор.")
            dividerVisible: false
            MeetySpinBox {
                id: threadsSpin
                Layout.fillWidth: true
                from: 0; to: 64
                onValueModified: { panel.t().n_threads = value; scr.touch() }
            }
        }
    }

    component FolderChip: AbstractButton {
        id: chip
        property string folderPath: ""

        implicitWidth: chipContent.implicitWidth + leftPadding + rightPadding
        implicitHeight: 36
        hoverEnabled: true
        activeFocusOnTab: true
        leftPadding: 10
        rightPadding: 10

        background: Rectangle {
            radius: Theme.rMd
            color: chip.hovered ? Theme.paper : Theme.paperSub
            border.width: 1
            border.color: chip.activeFocus ? Theme.focus : chip.hovered ? Theme.rule2 : Theme.rule
            Behavior on color { ColorAnimation { duration: Theme.durBase } }
        }
        contentItem: RowLayout {
            id: chipContent
            spacing: 8
            MeetyIcon { name: "folder"; size: 14; color: chip.hovered ? Theme.ink : Theme.ink2 }
            Text {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: chip.folderPath
                elide: Text.ElideLeft
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsSmall
                font.weight: Theme.wMedium
                color: chip.hovered ? Theme.ink : Theme.ink2
            }
            Rectangle {
                Layout.preferredWidth: 1
                Layout.preferredHeight: 18
                color: Theme.rule
            }
            MeetyIcon { name: "edit"; size: 12; color: Theme.ink3 }
        }
    }

    component ModelSubLabel: Item {
        property string label: ""
        property int count: 0
        property bool topDivider: false
        implicitHeight: labelRow.implicitHeight + (topDivider ? 20 : 16)
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 1
            color: Theme.rule
            visible: parent.topDivider
        }
        RowLayout {
            id: labelRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.leftMargin: 10
            anchors.rightMargin: 10
            anchors.bottomMargin: 6
            Text {
                Layout.fillWidth: true
                text: label
                font.family: Theme.fontUi
                font.pixelSize: Theme.fsMicro
                font.weight: Theme.wSemiBold
                font.letterSpacing: Theme.tracking(Theme.fsMicro, 0.12)
                color: Theme.ink3
            }
            Text {
                text: count
                font.family: Theme.fontMono
                font.pixelSize: Theme.fsMicro
                color: Theme.ink4
            }
        }
    }

    component InstalledCatalogRow: Rectangle {
        id: row
        property var modelInfo
        property bool active: false
        signal select()
        signal deleteRequested()

        implicitHeight: content.implicitHeight + 22
        radius: Theme.rSm
        color: active ? Theme.paper : hover.containsMouse ? Theme.paper3 : "transparent"
        border.width: active ? 1 : 0
        border.color: Theme.rule

        MouseArea {
            id: hover
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: row.active ? Qt.ArrowCursor : Qt.PointingHandCursor
            onClicked: if (!row.active) row.select()
        }

        RowLayout {
            id: content
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: 8
            anchors.rightMargin: 12
            spacing: 14
            Item {
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
                Rectangle {
                    anchors.centerIn: parent
                    width: 12
                    height: 12
                    radius: 6
                    color: row.active ? Theme.accent : Theme.paper
                    border.width: row.active ? 0 : 1
                    border.color: hover.containsMouse ? Theme.ink3 : Theme.rule2
                    Rectangle {
                        anchors.centerIn: parent
                        width: 18
                        height: 18
                        radius: 9
                        color: "transparent"
                        border.width: 3
                        border.color: Theme.accentTint
                        visible: row.active
                    }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: 3
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: row.modelInfo.display_name
                        elide: Text.ElideRight
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBodyLg
                        font.weight: Theme.wSemiBold
                        color: Theme.ink
                    }
                    MeetyTag { mono: true; text: row.modelInfo.approximate_size; color: Theme.paper3 }
                    MeetyTag {
                        visible: row.active
                        text: qsTr("Активна")
                        color: Theme.accentTint
                    }
                }
                Text {
                    Layout.fillWidth: true
                    text: row.modelInfo.description
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
            }
            MeetyIconButton {
                visible: !row.active
                iconName: "trash"
                iconSize: 13
                onClicked: row.deleteRequested()
            }
        }
    }

    component AvailableCatalogRow: Rectangle {
        id: row
        property var modelInfo
        property string jobId: ""
        property string installStatus: ""
        property real progressPercent: -1
        property string installError: ""
        property bool installing: installStatus === "queued" || installStatus === "downloading"

        implicitHeight: content.implicitHeight + 22
        radius: Theme.rSm
        color: "transparent"
        opacity: installing ? 1 : 0.78

        RowLayout {
            id: content
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: 8
            anchors.rightMargin: 12
            spacing: 14
            Item { Layout.preferredWidth: 24; Layout.preferredHeight: 24 }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: 3
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: row.modelInfo.display_name
                        elide: Text.ElideRight
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBodyLg
                        font.weight: Theme.wSemiBold
                        color: Theme.ink2
                    }
                    MeetyTag { mono: true; text: row.modelInfo.approximate_size; color: Theme.paper3 }
                }
                Text {
                    Layout.fillWidth: true
                    text: row.modelInfo.description
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
                RowLayout {
                    Layout.fillWidth: true
                    visible: row.installing || row.installStatus === "failed"
                    spacing: 10
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 4
                        radius: 4
                        color: Theme.paper3
                        clip: true
                        Rectangle {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: row.progressPercent >= 0 ? parent.width * row.progressPercent / 100 : 0
                            radius: 4
                            color: Theme.accent
                        }
                    }
                    Text {
                        text: row.installStatus === "failed"
                              ? qsTr("Ошибка")
                              : row.progressPercent >= 0
                                ? qsTr("%1%").arg(Math.round(row.progressPercent))
                                : qsTr("…")
                        font.family: Theme.fontMono
                        font.pixelSize: Theme.fsMicro
                        color: row.installStatus === "failed" ? Theme.rec : Theme.ink3
                    }
                }
            }
            MeetyButton {
                iconName: "download"
                text: row.installStatus === "failed" ? qsTr("Повторить") : qsTr("Скачать")
                enabled: !row.installing
                visible: !row.installing
                onClicked: installReq.post("/api/v1/transcription-models/" + row.modelInfo.id + "/install", ({}))
            }
            MeetyButton {
                text: qsTr("Идёт")
                enabled: false
                visible: row.installing
            }
        }

        Request {
            id: installReq
            onOk: function(json) {
                row.jobId = json.job_id || ""
                row.installStatus = "queued"
                row.progressPercent = -1
                row.installError = ""
                pollTimer.restart()
            }
            onFail: function(s, e) {
                row.installStatus = "failed"
                row.installError = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
            }
        }
        Request {
            id: pollReq
            onOk: function(json) {
                row.installStatus = json.status || ""
                row.progressPercent = json.percent === undefined || json.percent === null ? -1 : json.percent
                row.installError = json.error || ""
                if (row.installStatus === "done" || row.installStatus === "failed") {
                    pollTimer.stop()
                    panel.refreshModels()
                }
            }
            onFail: function(s, e) {
                row.installStatus = "failed"
                row.installError = s > 0 ? qsTr("HTTP %1: %2").arg(s).arg(e) : e
                pollTimer.stop()
            }
        }
        Timer {
            id: pollTimer
            interval: 700
            repeat: true
            onTriggered: {
                if (row.jobId.length === 0 || pollReq.busy)
                    return
                pollReq.get("/api/v1/transcription-models/installations/" + row.jobId)
            }
        }
    }

    component CustomModelRow: Rectangle {
        id: row
        property var modelInfo
        property bool active: false
        signal select()
        signal remove()

        implicitHeight: content.implicitHeight + 22
        radius: Theme.rSm
        color: active ? Theme.paper : hover.containsMouse ? Theme.paper3 : "transparent"
        border.width: active ? 1 : 0
        border.color: Theme.rule

        MouseArea {
            id: hover
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: row.active ? Qt.ArrowCursor : Qt.PointingHandCursor
            onClicked: if (!row.active) row.select()
        }

        RowLayout {
            id: content
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: 8
            anchors.rightMargin: 12
            spacing: 14
            Item {
                Layout.preferredWidth: 24
                Layout.preferredHeight: 24
                Rectangle {
                    anchors.centerIn: parent
                    width: 12
                    height: 12
                    radius: 6
                    color: row.active ? Theme.accent : Theme.paper
                    border.width: row.active ? 0 : 1
                    border.color: hover.containsMouse ? Theme.ink3 : Theme.rule2
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: 3
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Text {
                        Layout.fillWidth: true
                        Layout.minimumWidth: 0
                        text: row.modelInfo.name || panel.nameFromPath(row.modelInfo.path)
                        elide: Text.ElideRight
                        font.family: Theme.fontUi
                        font.pixelSize: Theme.fsBodyLg
                        font.weight: Theme.wSemiBold
                        color: Theme.ink
                    }
                    MeetyTag {
                        visible: row.active
                        text: qsTr("Активна")
                        color: Theme.accentTint
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: !!row.modelInfo.description
                    text: row.modelInfo.description || ""
                    wrapMode: Text.WordWrap
                    font.family: Theme.fontUi
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
                Text {
                    Layout.fillWidth: true
                    text: row.modelInfo.path
                    elide: Text.ElideLeft
                    font.family: Theme.fontMono
                    font.pixelSize: Theme.fsSmall
                    color: Theme.ink3
                }
            }
            MeetyIconButton {
                visible: !row.active
                iconName: "trash"
                iconSize: 13
                onClicked: row.remove()
            }
        }
    }

    component AddFieldRow: ColumnLayout {
        property string label: ""
        property bool optional: false
        property bool requiredText: false
        default property alias content: slot.data
        Layout.fillWidth: true
        spacing: 4
        Text {
            Layout.fillWidth: true
            text: parent.label + (parent.optional ? qsTr(" (опц.)") : "")
            font.family: Theme.fontUi
            font.pixelSize: Theme.fsSmall
            font.weight: Theme.wSemiBold
            color: parent.requiredText ? Theme.ink2 : Theme.ink3
        }
        RowLayout {
            id: slot
            Layout.fillWidth: true
            spacing: 8
        }
    }
}
