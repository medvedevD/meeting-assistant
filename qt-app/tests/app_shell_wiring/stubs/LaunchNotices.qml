// Stands in for the launch notices, which talk to the sidecar and are tested
// in tests/first_run_steps.
import QtQuick

Item {
    property var meetings
    property var preferences
    signal openMeeting(var meeting)
    signal openSettings(int category)
}
