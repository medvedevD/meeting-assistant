// Stub for the AppShell wiring test: AppShell only needs these types to
// resolve at load time — every one lives inside a lazy Component and is
// never instantiated by the test.
import QtQuick

Item {
    property var shell
    property var store
    property string meetingId
}
