cmake_minimum_required(VERSION 3.21)

project(meeting-assistant-qt
    VERSION 0.1.0
    DESCRIPTION "Meeting Assistant — Qt Quick GUI (sidecar HTTP client)"
    LANGUAGES CXX)

# Qt 6.7+ is required: QRestAccessManager / the cleaner reply-error split land
# in 6.7. We deliberately use plain C++ Qt + a SEPARATE Rust sidecar binary —
# there is NO cxx-qt / qt-build-utils here (the boundary is loopback HTTP, not
# FFI).
find_package(Qt6 6.7 REQUIRED COMPONENTS Quick QuickControls2 Network)

qt_standard_project_setup(REQUIRES 6.7)

# QML files live in qml/ (an extra dir); let Qt generate the qmldir for it.
if(QT_KNOWN_POLICY_QTP0004)
    qt_policy(SET QTP0004 NEW)
endif()

set(CMAKE_AUTOMOC ON)

# ── Generated IPC-protocol header (single source of truth = Rust) ────────────
set(_rust_api_lib "${CMAKE_CURRENT_SOURCE_DIR}/../rust/crates/api/src/lib.rs")
set(_gen_dir      "${CMAKE_CURRENT_BINARY_DIR}/generated")
set(_gen_header   "${_gen_dir}/ClientProtocol.h")
file(MAKE_DIRECTORY "${_gen_dir}")

add_custom_command(
    OUTPUT  "${_gen_header}"
    COMMAND ${CMAKE_COMMAND}
            -DRUST_LIB=${_rust_api_lib}
            -DOUT=${_gen_header}
            -P ${CMAKE_CURRENT_SOURCE_DIR}/cmake/GenClientProtocol.cmake
    DEPENDS "${_rust_api_lib}"
            "${CMAKE_CURRENT_SOURCE_DIR}/cmake/GenClientProtocol.cmake"
    COMMENT "Generating ClientProtocol.h from Rust PROTOCOL_VERSION"
    VERBATIM)
add_custom_target(gen_client_protocol DEPENDS "${_gen_header}")

# ── Sidecar location (built separately by cargo, NOT by CMake) ───────────────
# The Rust helper is its own `cargo build` artefact. Packaging copies it in
# beside the GUI on every OS (macOS: Contents/MacOS, Linux: AppDir bin,
# Windows: install dir) so SidecarManager finds it via applicationDirPath().
# Both binaries are built from the same revision and version-pinned together.
set(MEETING_SERVER_BIN
    "${CMAKE_CURRENT_SOURCE_DIR}/../rust/target/release/meeting-server"
    CACHE FILEPATH "Path to the cargo-built meeting-server sidecar to bundle")
if(WIN32)
    set(_sidecar_installed_name "meeting-server.exe")
else()
    set(_sidecar_installed_name "meeting-server")
endif()

# ── GUI executable ───────────────────────────────────────────────────────────
qt_add_executable(meeting-assistant-qt
    src/main.cpp
    src/SidecarManager.h   src/SidecarManager.cpp
    src/ApiClient.h        src/ApiClient.cpp
    src/JobPoller.h        src/JobPoller.cpp
    "${_gen_header}")

# macOS: ship as MeetingAssistant.app. The Info.plist carries the
# NSScreenCaptureUsageDescription that section-05's ScreenCaptureKit path
# requires (without it macOS kills the process with no TCC prompt).
#
# OFF by default so the section-03 dev workflow (run-qt.sh expects a plain
# `meeting-assistant-qt` executable) is untouched. packaging/macos/build-app.sh
# configures with -DMA_MACOS_APP_BUNDLE=ON for the real artefact.
option(MA_MACOS_APP_BUNDLE "Build MeetingAssistant.app bundle (packaging)" OFF)
if(APPLE AND MA_MACOS_APP_BUNDLE)
    set(_app_icns "${CMAKE_CURRENT_SOURCE_DIR}/../packaging/assets/meeting-assistant.icns")
    target_sources(meeting-assistant-qt PRIVATE "${_app_icns}")
    set_source_files_properties("${_app_icns}" PROPERTIES
        MACOSX_PACKAGE_LOCATION "Resources")
    set(MACOSX_BUNDLE_BUNDLE_NAME            "Meeting Assistant")
    set(MACOSX_BUNDLE_EXECUTABLE_NAME        "MeetingAssistant")
    set(MACOSX_BUNDLE_GUI_IDENTIFIER         "com.meeting-assistant.app")
    set(MACOSX_BUNDLE_BUNDLE_VERSION         "${PROJECT_VERSION}")
    set(MACOSX_BUNDLE_SHORT_VERSION_STRING   "${PROJECT_VERSION}")
    set(MACOSX_BUNDLE_ICON_FILE              "meeting-assistant.icns")
    set_target_properties(meeting-assistant-qt PROPERTIES
        MACOSX_BUNDLE TRUE
        MACOSX_BUNDLE_INFO_PLIST
            "${CMAKE_CURRENT_SOURCE_DIR}/../packaging/macos/Info.plist.in"
        OUTPUT_NAME "MeetingAssistant")
endif()

add_dependencies(meeting-assistant-qt gen_client_protocol)

target_include_directories(meeting-assistant-qt PRIVATE "${_gen_dir}")

# SettingsStore is a QML singleton (decision #12): one shared server-settings
# cache reachable from any screen as `SettingsStore`.
set_source_files_properties(qml/SettingsStore.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE)

# Theme is the redesign's design-token singleton (qt-redesign Phase 1):
# reachable from any screen as `Theme`.
set_source_files_properties(qml/theme/Theme.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE)

qt_add_qml_module(meeting-assistant-qt
    URI MeetingAssistant
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/AppShell.qml
        qml/MeetingStore.qml
        qml/SettingsStore.qml
        qml/Request.qml
        qml/screens/MeetingListScreen.qml
        qml/screens/WelcomeScreen.qml
        qml/screens/NewRecordingScreen.qml
        qml/screens/GenerateProtocolScreen.qml
        qml/screens/MeetingDetailScreen.qml
        qml/screens/MeetingNoProtocolScreen.qml
        qml/screens/SettingsScreen.qml
        qml/screens/DiagnosticsScreen.qml
        qml/screens/settings/WhisperPanel.qml
        qml/screens/settings/LlmPanel.qml
        qml/screens/settings/TemplatesPanel.qml
        qml/screens/settings/StoragePanel.qml
        qml/screens/settings/RecordingPanel.qml
        qml/components/PipelineProgress.qml
        qml/components/MeetyButton.qml
        qml/components/MeetyComboBox.qml
        qml/components/MeetyField.qml
        qml/components/MeetySpinBox.qml
        qml/components/MeetySwitch.qml
        qml/components/MeetySegmented.qml
        qml/components/MeetyTag.qml
        qml/components/MeetyCard.qml
        qml/components/MeetySectionLabel.qml
        qml/components/MeetyMenu.qml
        qml/components/MeetyMenuItem.qml
        qml/components/MeetyDialog.qml
        qml/components/MeetyDialogActions.qml
        qml/components/MeetyCheckDelegate.qml
        qml/components/MeetyToolTip.qml
        qml/components/MeetyIcon.qml
        qml/components/MeetyIconButton.qml
        qml/components/MeetyWordmark.qml
        qml/components/SettingsRow.qml
        qml/components/ProtocolDocument.qml
        qml/theme/Theme.qml
        qml/theme/ThemeGallery.qml
        qml/i18n/errors.js)

# Bundled design fonts (qt-redesign): variable TTFs registered as :/fonts/*,
# loaded by Theme.qml's FontLoaders. All OFL-licensed (see fonts/OFL-*.txt);
# packaging must ship the OFL notices and the binary grows accordingly.
qt_add_resources(meeting-assistant-qt "meety_fonts"
    PREFIX "/"
    FILES
        fonts/Geist-Variable.ttf
        fonts/Newsreader-Variable.ttf
        fonts/Newsreader-Italic-Variable.ttf
        fonts/JetBrainsMono-Variable.ttf)

# Fusion fallback: compiled-in `:/qtquickcontrols2.conf`. Qt Quick Controls
# reads this at startup; combined with QQuickStyle::setStyle("Fusion") in
# main.cpp it makes Fusion non-overridable. We never link/ship the Material or
# Universal style plugins.
qt_add_resources(meeting-assistant-qt "controls_conf"
    PREFIX "/"
    FILES qtquickcontrols2.conf)

target_link_libraries(meeting-assistant-qt PRIVATE
    Qt6::Quick
    Qt6::QuickControls2
    Qt6::Network)

if(WIN32)
    # Job Object APIs for kernel-enforced sidecar reaping on parent death.
    target_link_libraries(meeting-assistant-qt PRIVATE kernel32)
endif()

# ── Install rules (section-07 packaging) ─────────────────────────────────────
# Lay out the two-binary bundle so SidecarManager::locateSidecar() always finds
# meeting-server next to the GUI (it resolves applicationDirPath()):
#   macOS    MeetingAssistant.app/Contents/MacOS/{MeetingAssistant,meeting-server}
#   Linux    <prefix>/bin/{meeting-assistant-qt,meeting-server}
#   Windows  <prefix>/{meeting-assistant-qt.exe,meeting-server.exe}
# The platform deploy tool (macdeployqt / linuxdeploy / windeployqt) adds the
# Qt runtime afterwards; none of them bundle the helper — that is this rule.
set(_lic_dir "${CMAKE_CURRENT_SOURCE_DIR}/../packaging/LICENSES")

if(APPLE AND MA_MACOS_APP_BUNDLE)
    install(TARGETS meeting-assistant-qt BUNDLE DESTINATION .)
    # macdeployqt does NOT copy the helper — install it into Contents/MacOS.
    install(PROGRAMS "${MEETING_SERVER_BIN}"
        DESTINATION "MeetingAssistant.app/Contents/MacOS")
    # LGPLv3: written offer + relink notes travel inside the artefact.
    install(DIRECTORY "${_lic_dir}/"
        DESTINATION "MeetingAssistant.app/Contents/Resources/licenses")
elseif(WIN32)
    install(TARGETS meeting-assistant-qt RUNTIME DESTINATION .)
    install(PROGRAMS "${MEETING_SERVER_BIN}" DESTINATION "."
        RENAME "${_sidecar_installed_name}")
    install(DIRECTORY "${_lic_dir}/" DESTINATION "licenses")
else() # Linux (AppImage AppDir / generic prefix)
    install(TARGETS meeting-assistant-qt RUNTIME DESTINATION bin)
    install(PROGRAMS "${MEETING_SERVER_BIN}" DESTINATION bin
        RENAME "${_sidecar_installed_name}")
    install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../packaging/linux/meeting-assistant.desktop"
        DESTINATION share/applications)
    install(DIRECTORY "${_lic_dir}/" DESTINATION share/doc/meeting-assistant/licenses)
endif()

# ── Offscreen Qt smoke test (section-08) ─────────────────────────────────────
# OFF by default so the section-03 dev workflow and section-07 packaging build
# only the GUI. The section-08 CI matrix configures with -DMA_BUILD_TESTS=ON
# and runs it via ctest under QT_QPA_PLATFORM=offscreen.
option(MA_BUILD_TESTS "Build the offscreen Qt boot smoke test (section-08)" OFF)
if(MA_BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()
