# Offscreen Qt boot smoke (section-08). Compiles the real GUI controller
# sources (SidecarManager + ApiClient — the same files main.cpp builds, so no
# logic can drift between the shipped app and the test) plus the generated
# ClientProtocol.h, and runs them headlessly against a real meeting-server.

find_package(Qt6 6.7 REQUIRED COMPONENTS Test Gui Qml Quick QuickControls2 Network)

qt_add_executable(tst_job_poller
    tst_job_poller.cpp
    ../src/ApiClient.h   ../src/ApiClient.cpp
    ../src/JobPoller.h   ../src/JobPoller.cpp)

target_include_directories(tst_job_poller PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../src")

target_link_libraries(tst_job_poller PRIVATE Qt6::Test Qt6::Network)

add_test(NAME qt_job_poller_terminal_done COMMAND tst_job_poller)
set_tests_properties(qt_job_poller_terminal_done PROPERTIES
    TIMEOUT 30)

qt_add_executable(tst_pipeline_progress
    tst_pipeline_progress.cpp
    ../src/ApiClient.h   ../src/ApiClient.cpp
    ../src/JobPoller.h   ../src/JobPoller.cpp)

target_include_directories(tst_pipeline_progress PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../src")
target_compile_definitions(tst_pipeline_progress PRIVATE
    MA_PIPELINE_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/pipeline_qml"
    MA_PIPELINE_QML_FILE="${CMAKE_CURRENT_BINARY_DIR}/pipeline_qml/MeetingAssistant/qml/components/PipelineProgress.qml")

set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml"
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/PipelineProgress.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/PipelineProgress.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyButton.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/MeetyButton.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyIcon.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/MeetyIcon.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/theme/Theme.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/i18n/errors.js"
    PROPERTIES QT_RESOURCE_ALIAS "qml/i18n/errors.js")

qt_add_qml_module(tst_pipeline_progress
    URI MeetingAssistant
    VERSION 1.0
    NO_CACHEGEN
    OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/pipeline_qml/MeetingAssistant"
    QML_FILES
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/PipelineProgress.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyButton.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyIcon.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/i18n/errors.js")

target_link_libraries(tst_pipeline_progress PRIVATE
    Qt6::Test Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::Network)

add_test(NAME qt_pipeline_progress_pending_done COMMAND tst_pipeline_progress)
set_tests_properties(qt_pipeline_progress_pending_done PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)

qt_add_executable(tst_protocol_document
    tst_protocol_document.cpp)

target_compile_definitions(tst_protocol_document PRIVATE
    MA_PROTOCOL_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/protocol_qml"
    MA_PROTOCOL_QML_FILE="${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/ProtocolDocument.qml")

set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml"
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/ProtocolDocument.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/ProtocolDocument.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyIcon.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/MeetyIcon.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/theme/Theme.qml")

qt_add_qml_module(tst_protocol_document
    URI MeetingAssistant
    VERSION 1.0
    NO_CACHEGEN
    OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/protocol_qml/MeetingAssistant"
    QML_FILES
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/ProtocolDocument.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyIcon.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml")

target_link_libraries(tst_protocol_document PRIVATE
    Qt6::Test Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2)

add_test(NAME qt_protocol_document_markdown_blocks COMMAND tst_protocol_document)
set_tests_properties(qt_protocol_document_markdown_blocks PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)

qt_add_executable(tst_meety_menu
    tst_meety_menu.cpp)

target_compile_definitions(tst_meety_menu PRIVATE
    MA_MENU_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/menu_qml")

set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyMenu.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/MeetyMenu.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyMenuItem.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/MeetyMenuItem.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyIcon.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/components/MeetyIcon.qml")
set_source_files_properties(
    "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml"
    PROPERTIES QT_RESOURCE_ALIAS "qml/theme/Theme.qml")

qt_add_qml_module(tst_meety_menu
    URI MeetingAssistant
    VERSION 1.0
    NO_CACHEGEN
    OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/menu_qml/MeetingAssistant"
    QML_FILES
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyMenu.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyMenuItem.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/components/MeetyIcon.qml"
        "${CMAKE_CURRENT_SOURCE_DIR}/../qml/theme/Theme.qml")

target_link_libraries(tst_meety_menu PRIVATE
    Qt6::Test Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2)

add_test(NAME qt_meety_menu_long_items_expand COMMAND tst_meety_menu)
set_tests_properties(qt_meety_menu_long_items_expand PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)

qt_add_executable(tst_smoke
    tst_smoke.cpp
    ../src/SidecarManager.h ../src/SidecarManager.cpp
    ../src/ApiClient.h      ../src/ApiClient.cpp
    "${_gen_header}")

# Same generated single-source-of-truth header the GUI uses.
add_dependencies(tst_smoke gen_client_protocol)
target_include_directories(tst_smoke PRIVATE
    "${_gen_dir}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../src")

target_link_libraries(tst_smoke PRIVATE Qt6::Test Qt6::Gui Qt6::Network)

# Default = the cargo release artefact (matches MEETING_SERVER_BIN's profile).
# CI overrides with -DMA_SMOKE_SIDECAR=<path> for its build profile / .exe.
if(WIN32)
    set(_smoke_sidecar_default
        "${CMAKE_CURRENT_SOURCE_DIR}/../../rust/target/release/meeting-server.exe")
else()
    set(_smoke_sidecar_default
        "${CMAKE_CURRENT_SOURCE_DIR}/../../rust/target/release/meeting-server")
endif()
set(MA_SMOKE_SIDECAR "${_smoke_sidecar_default}"
    CACHE FILEPATH "meeting-server binary the offscreen smoke test spawns")

add_test(NAME qt_offscreen_smoke COMMAND tst_smoke)
set_tests_properties(qt_offscreen_smoke PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen;MA_SIDECAR=${MA_SMOKE_SIDECAR}"
    TIMEOUT 180)

# Regression for the transcription-progress-visibility task: QML must not
# call the (now removed) synchronous POST /api/v1/transcribe shortcut — it
# produced an indeterminate spinner on long meetings. The transcribe step now
# goes through POST /api/v1/meetings/:id/reprocess {kind:"transcribe"}, polled
# by the same PipelineProgress component as the protocol step.
add_test(NAME qt_no_sync_transcribe_in_qml
    COMMAND ${CMAKE_COMMAND} -DSEARCH_DIR=${CMAKE_CURRENT_SOURCE_DIR}/../qml
                             -P ${CMAKE_CURRENT_SOURCE_DIR}/no_sync_transcribe.cmake)

# Same rationale as the main GUI target (see ../CMakeLists.txt): these test
# executables bundle a QML module, so the finalizer's static plugin-import scan
# warns that style-impl plugin targets (Fusion/Basic/Material/Shapes/…) don't
# exist in scope — noise for a shared Qt that loads plugins at runtime. Skip
# the scan for shared Qt; keep it for a static build where they must be linked.
if(QT6_IS_SHARED_LIBS_BUILD)
    set_target_properties(
        tst_pipeline_progress tst_protocol_document tst_meety_menu
        PROPERTIES QT_QML_MODULE_NO_IMPORT_SCAN ON)
endif()
