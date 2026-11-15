// Stands in for the first-run setup steps, which talk to the sidecar and have
// their own test (tests/first_run_steps).
import QtQuick

Item {
    property var preferences
    signal startRecording()
    signal importRecording()
    signal skipped()
    signal openSettings(int category)
    implicitHeight: 1
}
