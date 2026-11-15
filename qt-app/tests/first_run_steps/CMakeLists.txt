# ── First-run setup steps and launch notices ────────────────────────────────────────────────────
# The real FirstRunSteps with the real stores and ApiClient, against a scripted
# HTTP server. See tst_first_run_steps.cpp for what is stubbed.
qt_add_executable(tst_first_run_steps
    tst_first_run_steps.cpp
    ../../src/ApiClient.h ../../src/ApiClient.cpp)

target_include_directories(tst_first_run_steps PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src")
target_compile_definitions(tst_first_run_steps PRIVATE
    MA_FIRST_RUN_STEPS_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports")

foreach(_singleton AudioDevicesStore SettingsStore SetupStore)
    set_source_files_properties("${CMAKE_CURRENT_SOURCE_DIR}/../../qml/${_singleton}.qml"
        PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
endforeach()

ma_add_test_qml_module(tst_first_run_steps
    APP_QML
        AudioDevicesStore.qml
        Request.qml
        SettingsStore.qml
        SetupStore.qml
        components/MeetyButton.qml
        components/MeetyComboBox.qml
        components/MeetyField.qml
        components/MeetyIcon.qml
        components/MeetyIconButton.qml
        components/MeetyToolTip.qml
        components/MeetyLevelMeter.qml
        i18n/errors.js
        screens/welcome/FirstRunSteps.qml
        screens/welcome/LaunchNotices.qml
        screens/welcome/SetupStepRow.qml
        theme/Theme.qml)

target_link_libraries(tst_first_run_steps PRIVATE Qt6::Network)

add_test(NAME qt_first_run_steps COMMAND tst_first_run_steps)
set_tests_properties(qt_first_run_steps PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 120)
