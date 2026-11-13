# ── Settings switches save every change of `checked` ─────────────────────────
# The real Recording and Transcription panels on a stand-in settings screen,
# switched by mouse, keyboard and the accessibility actions VoiceOver uses.
qt_add_executable(tst_settings_switches
    tst_settings_switches.cpp)

target_compile_definitions(tst_settings_switches PRIVATE
    MA_SETTINGS_SWITCHES_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports")

set_source_files_properties("${CMAKE_CURRENT_SOURCE_DIR}/../../qml/AudioDevicesStore.qml"
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE)

ma_add_test_qml_module(tst_settings_switches
    APP_QML
        AudioDevicesStore.qml
        Request.qml
        components/MeetyButton.qml
        components/MeetyComboBox.qml
        components/MeetyDialog.qml
        components/MeetyDialogActions.qml
        components/MeetyField.qml
        components/MeetyIcon.qml
        components/MeetyIconButton.qml
        components/MeetyLevelMeter.qml
        components/MeetySegmented.qml
        components/MeetySpinBox.qml
        components/MeetySwitch.qml
        components/MeetyTag.qml
        components/MeetyToolTip.qml
        components/SettingsRow.qml
        screens/settings/RecordingPanel.qml
        screens/settings/WhisperPanel.qml
        theme/Theme.qml)

add_test(NAME qt_settings_switches_save_every_change COMMAND tst_settings_switches)
set_tests_properties(qt_settings_switches_save_every_change PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
