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

# ── QML tests ────────────────────────────────────────────────────────────────
# Each one bundles its own cut of the real module, which has to keep the URI
# MeetingAssistant because the shipped files `import MeetingAssistant`.
# qt_add_qml_module() names the module's generated resource after the URI
# (.qt/rcc/qmake_MeetingAssistant.qrc under the current binary dir), so two such
# targets in one CMake directory share that file and both embed the last
# target's qmldir. Hence one directory per target; EmbeddedQmldirGuard.h is what
# catches a target that ends up sharing one anyway.
#
# Usage, from the target's own directory:
#   ma_add_test_qml_module(<target>
#       APP_QML   <paths relative to qt-app/qml>
#       LOCAL_QML <test-only QML files relative to the calling directory>)
#
# It must run in that directory, not here: source-file properties are
# directory-scoped, and the executable's finalizer, which reads
# QT_QML_MODULE_NO_IMPORT_SCAN, runs when that directory ends.
function(ma_add_test_qml_module target)
    cmake_parse_arguments(PARSE_ARGV 1 arg "" "" "APP_QML;LOCAL_QML")

    set(_app_qml_dir "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../qml")
    set_source_files_properties("${_app_qml_dir}/theme/Theme.qml"
        PROPERTIES QT_QML_SINGLETON_TYPE TRUE)

    set(_app_qml)
    foreach(_file IN LISTS arg_APP_QML)
        set_source_files_properties("${_app_qml_dir}/${_file}"
            PROPERTIES QT_RESOURCE_ALIAS "qml/${_file}")
        list(APPEND _app_qml "${_app_qml_dir}/${_file}")
    endforeach()

    qt_add_qml_module(${target}
        URI MeetingAssistant
        VERSION 1.0
        NO_CACHEGEN
        OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/imports/MeetingAssistant"
        QML_FILES ${_app_qml} ${arg_LOCAL_QML})

    target_include_directories(${target} PRIVATE
        "${CMAKE_CURRENT_FUNCTION_LIST_DIR}")
    target_link_libraries(${target} PRIVATE
        Qt6::Test Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2)

    # Same rationale as the main GUI target (see ../CMakeLists.txt): the
    # finalizer's static plugin-import scan warns that style-impl plugin targets
    # (Fusion/Basic/Material/Shapes/…) don't exist in scope — noise for a shared
    # Qt that loads plugins at runtime. Skip the scan for shared Qt; keep it for
    # a static build where they must be linked.
    if(QT6_IS_SHARED_LIBS_BUILD)
        set_target_properties(${target} PROPERTIES
            QT_QML_MODULE_NO_IMPORT_SCAN ON)
    endif()
endfunction()

add_subdirectory(pipeline_progress)
add_subdirectory(protocol_document)
add_subdirectory(meety_menu)
add_subdirectory(first_run)
add_subdirectory(app_shell_wiring)
add_subdirectory(i18n)

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
