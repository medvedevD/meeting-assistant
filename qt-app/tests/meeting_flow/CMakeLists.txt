# ── Screen flows around a job (generate, import) ─────────────────────────────
# The real AppShell, screens and stores with the real ApiClient and JobPoller,
# against a scripted HTTP server. See tst_meeting_flow.cpp for what is stubbed.
qt_add_executable(tst_meeting_flow
    tst_meeting_flow.cpp
    ../../src/ApiClient.h   ../../src/ApiClient.cpp
    ../../src/JobPoller.h   ../../src/JobPoller.cpp
    ../../src/PathUtils.h)

target_include_directories(tst_meeting_flow PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src")
target_compile_definitions(tst_meeting_flow PRIVATE
    MA_MEETING_FLOW_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports")

foreach(_singleton ActiveJobsStore AudioDevicesStore SettingsStore)
    set_source_files_properties("${CMAKE_CURRENT_SOURCE_DIR}/../../qml/${_singleton}.qml"
        PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
endforeach()

ma_add_test_qml_module(tst_meeting_flow
    APP_QML
        ActiveJobsStore.qml
        AppShell.qml
        AudioDevicesStore.qml
        MeetingStore.qml
        Request.qml
        SettingsStore.qml
        components/MeetyButton.qml
        components/MeetyCard.qml
        components/MeetyCheckDelegate.qml
        components/MeetyComboBox.qml
        components/MeetyDialog.qml
        components/MeetyDialogActions.qml
        components/MeetyField.qml
        components/MeetyIcon.qml
        components/MeetyIconButton.qml
        components/MeetyMenu.qml
        components/MeetyMenuItem.qml
        components/MeetySectionLabel.qml
        components/MeetySegmented.qml
        components/MeetySwitch.qml
        components/MeetyToolTip.qml
        components/MeetyWordmark.qml
        components/PipelineProgress.qml
        components/ProtocolDocument.qml
        i18n/errors.js
        screens/GenerateProtocolScreen.qml
        screens/MeetingDetailScreen.qml
        screens/MeetingNoProtocolScreen.qml
        screens/NewRecordingScreen.qml
        theme/Theme.qml
    LOCAL_QML
        stubs/AudioPlayer.qml
        stubs/DiagnosticsScreen.qml
        stubs/SettingsScreen.qml
        stubs/WelcomeScreen.qml)

target_link_libraries(tst_meeting_flow PRIVATE Qt6::Network)

add_test(NAME qt_meeting_flow COMMAND tst_meeting_flow)
set_tests_properties(qt_meeting_flow PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 120)
