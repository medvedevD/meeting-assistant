# ── AppShell → WelcomeScreen wiring (binding-loop regression) ────────────────
# Loads the REAL AppShell.qml so the guard tracks the shipped file. The six
# pushable screens are stubbed: each sits inside a lazy Component, so they only
# need to resolve as types, and stubbing keeps the staged type graph small.
# MeetingStore/Request resolve implicitly from AppShell.qml's own directory;
# everything under components/, screens/ and theme/ comes from the module below.
qt_add_executable(tst_app_shell_wiring tst_app_shell_wiring.cpp)

target_compile_definitions(tst_app_shell_wiring PRIVATE
    MA_APP_SHELL_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports"
    MA_APP_SHELL_QML_FILE="${CMAKE_CURRENT_SOURCE_DIR}/../../qml/AppShell.qml")

ma_add_test_qml_module(tst_app_shell_wiring
    APP_QML
        components/MeetyButton.qml
        components/MeetyDialog.qml
        components/MeetyDialogActions.qml
        components/MeetyField.qml
        components/MeetyIcon.qml
        components/MeetyIconButton.qml
        components/MeetySectionLabel.qml
        components/MeetyToolTip.qml
        components/MeetyWordmark.qml
        screens/WelcomeScreen.qml
        theme/Theme.qml
    LOCAL_QML
        stubs/DiagnosticsScreen.qml
        stubs/GenerateProtocolScreen.qml
        stubs/MeetingDetailScreen.qml
        stubs/MeetingNoProtocolScreen.qml
        stubs/NewRecordingScreen.qml
        stubs/SettingsScreen.qml)

add_test(NAME qt_app_shell_welcome_wiring COMMAND tst_app_shell_wiring)
set_tests_properties(qt_app_shell_welcome_wiring PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
