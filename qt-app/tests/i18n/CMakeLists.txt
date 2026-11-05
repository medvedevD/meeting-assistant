qt_add_executable(tst_i18n
    tst_i18n.cpp
    ../../src/ApiClient.h   ../../src/ApiClient.cpp
    ../../src/JobPoller.h   ../../src/JobPoller.cpp
    ../../src/Language.h    ../../src/Language.cpp)

target_include_directories(tst_i18n PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src")
target_compile_definitions(tst_i18n PRIVATE
    MA_I18N_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports"
    MA_I18N_QML_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports/MeetingAssistant/qml"
    MA_APP_QML_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../../qml"
    MA_PROMPTS_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../../../prompts")

ma_add_test_qml_module(tst_i18n
    APP_QML
        components/MeetyButton.qml
        components/MeetyComboBox.qml
        components/MeetyIcon.qml
        components/PipelineProgress.qml
        components/SettingsRow.qml
        i18n/errors.js
        screens/settings/LanguagePanel.qml
        theme/Theme.qml)

target_link_libraries(tst_i18n PRIVATE Qt6::Network)

# The app's translation, embedded at the path Language.cpp loads. Only lrelease:
# qt_add_translations would also add an lupdate target to `update_translations`,
# which would rewrite the .ts from this target's few sources.
qt_add_lrelease(tst_i18n
    TS_FILES ../../i18n/meeting-assistant_en.ts
    NO_GLOBAL_TARGET
    QM_FILES_OUTPUT_VARIABLE _i18n_qm_files)
set_source_files_properties(${_i18n_qm_files} PROPERTIES
    QT_RESOURCE_ALIAS "meeting-assistant_en.qm")
qt_add_resources(tst_i18n "i18n_translation"
    PREFIX "/i18n"
    FILES ${_i18n_qm_files})

add_test(NAME qt_i18n COMMAND tst_i18n)
set_tests_properties(qt_i18n PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
