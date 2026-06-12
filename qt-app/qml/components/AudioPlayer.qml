// AudioPlayer — minimal in-card playback for a meeting's recording.
// play/pause + a seek bar + elapsed/total time. Backed by Qt Multimedia's
// MediaPlayer; the source is a local file:// URL (GUI and recordings live on the
// same machine, so no streaming through the sidecar is needed).
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

    MediaPlayer {
        id: player
        source: root.source
        audioOutput: AudioOutput {}
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
            onClicked: player.playbackState === MediaPlayer.PlayingState
                       ? player.pause() : player.play()
        }

        Slider {
            id: seek
            Layout.fillWidth: true
            from: 0
            to: Math.max(1, player.duration)
            enabled: player.seekable
            // Follow playback, except while the user is dragging the handle
            // (then the self-reference keeps the dragged value).
            value: pressed ? value : player.position
            onMoved: player.position = value

            background: Rectangle {
                x: seek.leftPadding
                y: seek.topPadding + seek.availableHeight / 2 - height / 2
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
                width: 14
                height: 14
                radius: 7
                color: Theme.paper
                border.width: 2
                border.color: Theme.accent
            }
        }

        Text {
            text: root.fmt(player.position) + " / " + root.fmt(player.duration)
            font.family: Theme.fontMono
            font.pixelSize: Theme.fsSmall
            color: Theme.ink3
        }
    }

    // Inline failure (unsupported codec, missing file) instead of a dead button.
    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.hasError
        text: qsTr("Не удалось воспроизвести аудио")
        font.family: Theme.fontUi
        font.pixelSize: Theme.fsSmall
        color: Theme.warn
    }
}
