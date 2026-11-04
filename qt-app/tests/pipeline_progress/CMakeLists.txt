qt_add_executable(tst_pipeline_progress
    tst_pipeline_progress.cpp
    ../../src/ApiClient.h   ../../src/ApiClient.cpp
    ../../src/JobPoller.h   ../../src/JobPoller.cpp)

target_include_directories(tst_pipeline_progress PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src")
target_compile_definitions(tst_pipeline_progress PRIVATE
    MA_PIPELINE_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports"
    MA_PIPELINE_QML_FILE="${CMAKE_CURRENT_BINARY_DIR}/imports/MeetingAssistant/qml/components/PipelineProgress.qml")

ma_add_test_qml_module(tst_pipeline_progress
    APP_QML
        components/PipelineProgress.qml
        components/MeetyButton.qml
        components/MeetyIcon.qml
        theme/Theme.qml
        i18n/errors.js)

target_link_libraries(tst_pipeline_progress PRIVATE Qt6::Network)

add_test(NAME qt_pipeline_progress_pending_done COMMAND tst_pipeline_progress)
set_tests_properties(qt_pipeline_progress_pending_done PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
