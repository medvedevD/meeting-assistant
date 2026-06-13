// AudioPlayer — in-card playback for a meeting's recording.
// play/pause + seek bar + elapsed/total time + speed + volume. Backed by Qt
// Multimedia's MediaPlayer; the source is a local file:// URL (GUI and
// recordings live on the same machine, so no streaming through the sidecar).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtMultimedia
import MeetingAssistant

Item {
    id: root

    // file:// URL of the recording (build it with pathUtils.fileUrl(path)).
    property url source

    readonly property bool hasSource: source.toString().length > 0
    readonly property bool hasError: player.error !== MediaPlayer.NoError
    // Media is still resolving — no duration yet, controls show a loading hint.
    readonly property bool loading: hasSource && !hasError
                                    && (player.mediaStatus === MediaPlayer.LoadingMedia
                                        || player.mediaStatus === MediaPlayer.StalledMedia
                                        || player.mediaStatus === MediaPlayer.BufferingMedia)

    // Playback-speed cycle (no backend involvement; pure UI).
    readonly property var speeds: [1.0, 1.25, 1.5, 2.0]
    property int speedIndex: 0
    readonly property real speed: speeds[speedIndex]

    implicitHeight: 36

    // ms → "mm:ss"; clamps junk/NaN to 0 so the label never shows "NaN:NaN".
    function fmt(ms) {
        if (!ms || ms < 0 || isNaN(ms))
            ms = 0
        const total = Math.floor(ms / 1000)
        const m = Math.floor(total / 60)
        const s = total % 60
        return (m < 10 ? "0" : "") + m + ":" + (s < 10 ? "0" : "") + s
    }

    // play(), but restart from the top if the last play ran to the end (Qt
    // leaves position at the end, so a bare play() would be a no-op).
    function togglePlay() {
        if (player.playbackState === MediaPlayer.PlayingState) {
            player.pause()
            return
        }
        if (player.duration > 0 && player.position >= player.duration)
            player.position = 0
        player.play()
    }

    MediaPlayer {
        id: player
        source: root.source
        playbackRate: root.speed
        audioOutput: AudioOutput {
            id: out
            // Slider is the source of truth; output simply follows it.
            volume: volume_slider.value
        }
    }

    // Stop audio if the card/screen goes away while playing.
    Component.onDestruction: player.stop()

    RowLayout {
        id: controls
        anchors.fill: parent
        spacing: 12
        visible: !root.hasError

        MeetyIconButton {
            iconName: player.playbackState === MediaPlayer.PlayingState
                      ? "pause" : "play"
            enabled: root.hasSource
                     && player.mediaStatus !== MediaPlayer.InvalidMedia
            onClicked: root.togglePlay()
        }

        Slider {
            id: seek
            Layout.fillWidth: true
            from: 0
            to: player.duration > 0 ? player.duration : 1
            enabled: root.hasSource && !root.loading

            // Drive playback live while the user scrubs.
            onMoved: player.position = value

            // Follow playback, but only when the user is NOT dragging — pushing
            // position into `value` during a drag would fight the handle. A
            // plain assignment (not a binding on `value`) avoids the
            // self-referential binding that otherwise breaks on first drag.
            Connections {
                target: player
                function onPositionChanged() {
                    if (!seek.pressed)
                        seek.value = player.position
                }
            }

            background: Rectangle {
                x: seek.leftPadding
                y: seek.topPadding + seek.availableHeight / 2 - height / 2
                // implicit* give the Slider a sane height + hit area; without
                // them a custom handle/background can collapse to ~0px tall.
                implicitWidth: 200
                implicitHeight: 4
                width: seek.availableWidth
                height: 4
                radius: 2
                color: Theme.rule
                Rectangle {
                    width: seek.visualPosition * parent.width
                    height: parent.height
                    radius: 2
                    color: Theme.accent
                }
            }
            handle: Rectangle {
                x: seek.leftPadding + seek.visualPosition * (seek.availableWidth - width)
                y: seek.topPadding + seek.availableHeight / 2 - height / 2
                implicitWidth: 14
                implicitHeight: 14
                width: 14
                height: 14
                radius: 7
                color: Theme.paper
                border.width: 2
                border.color: Theme.accent
            }
        }

        Text {
            text: root.loading ? qsTr("Загрузка…")
                               : root.fmt(player.position) + " / " + root.fmt(player.duration)
            font.family: Theme.fontMono
            font.pixelSize: Theme.fsSmall
            color: Theme.ink3
        }

        // Playback speed — click to cycle 1× → 1.25× → 1.5× → 2× → 1×.
        MeetyButton {
            variant: "ghost"
            enabled: root.hasSource
            // Trim a trailing ".00"/".50" → "1×", "1.5×", "1.25×".
            text: Number(root.speed.toFixed(2)).toString().replace(/\.?0+$/, "") + "×"
            onClicked: root.speedIndex = (root.speedIndex + 1) % root.speeds.length
            MeetyToolTip { text: qsTr("Скорость воспроизведения"); visible: parent.hovered }
        }

        // Volume — compact track; AudioOutput.volume binds to this value.
        Slider {
            id: volume_slider
            Layout.preferredWidth: 72
            from: 0
            to: 1
            value: 1.0
            enabled: root.hasSource
            hoverEnabled: true

            background: Rectangle {
                x: volume_slider.leftPadding
                y: volume_slider.topPadding + volume_slider.availableHeight / 2 - height / 2
                implicitWidth: 72
                implicitHeight: 4
                width: volume_slider.availableWidth
                height: 4
                radius: 2
                color: Theme.rule
                Rectangle {
                    width: volume_slider.visualPosition * parent.width
                    height: parent.height
                    radius: 2
                    color: Theme.ink4
                }
            }
            handle: Rectangle {
                x: volume_slider.leftPadding
                   + volume_slider.visualPosition * (volume_slider.availableWidth - width)
                y: volume_slider.topPadding + volume_slider.availableHeight / 2 - height / 2
                implicitWidth: 12
                implicitHeight: 12
                width: 12
                height: 12
                radius: 6
                color: Theme.paper
                border.width: 2
                border.color: Theme.ink4
            }
            MeetyToolTip {
                text: qsTr("Громкость: %1%").arg(Math.round(volume_slider.value * 100))
                visible: volume_slider.hovered || volume_slider.pressed
            }
        }
    }

    // Inline failure (unsupported codec, missing file) instead of a dead button.
    // Prefer the backend's specific message when it gives one.
    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.hasError
        text: player.errorString.length > 0
              ? qsTr("Не удалось воспроизвести: %1").arg(player.errorString)
              : qsTr("Не удалось воспроизвести аудио")
        font.family: Theme.fontUi
        font.pixelSize: Theme.fsSmall
        color: Theme.warn
    }
}
