cmake_minimum_required(VERSION 3.21)

project(meeting-assistant-qt
    VERSION 0.1.0
    DESCRIPTION "Meeting Assistant — Qt Quick GUI (sidecar HTTP client)"
    LANGUAGES CXX)

# Qt 6.7+ is required: QRestAccessManager / the cleaner reply-error split land
# in 6.7. We deliberately use plain C++ Qt + a SEPARATE Rust sidecar binary —
# there is NO cxx-qt / qt-build-utils here (the boundary is loopback HTTP, not
# FFI).
# Multimedia powers the in-card recording player (MediaPlayer/AudioOutput). On
# Qt 6.7 it loads the ffmpeg media backend at runtime — the platform deploy
# tools bundle it, but its plugins must be covered by macOS signing/notarization
# and the AppImage/Windows layouts (see packaging/).
# LinguistTools provides lupdate/lrelease for the translations below.
find_package(Qt6 6.7 REQUIRED COMPONENTS
    Quick QuickControls2 Network Multimedia Svg LinguistTools)

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
    src/AppPreferences.h src/AppPreferences.cpp
    src/Language.h         src/Language.cpp
    src/Logging.h          src/Logging.cpp
    src/SidecarManager.h   src/SidecarManager.cpp
    src/ApiClient.h        src/ApiClient.cpp
    src/JobPoller.h        src/JobPoller.cpp
    src/PathUtils.h
    src/ScreenCapturePermission.h
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

# Windows: build as a GUI-subsystem app (WinMain) so launching from Explorer
# does not pop a console window next to the GUI. qt_add_executable does not set
# this by default, leaving a console-subsystem binary. Qt's Qt6::EntryPoint
# (linked via qt_standard_project_setup) provides the WinMain shim, so a plain
# int main() stays. The sidecar's own console is suppressed separately with
# CREATE_NO_WINDOW in SidecarManager. (No effect on macOS/Linux.)
if(WIN32)
    set_target_properties(meeting-assistant-qt PROPERTIES WIN32_EXECUTABLE TRUE)
endif()

target_include_directories(meeting-assistant-qt PRIVATE "${_gen_dir}")

# SettingsStore is a QML singleton (decision #12): one shared server-settings
# cache reachable from any screen as `SettingsStore`.
set_source_files_properties(qml/SettingsStore.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE)

# Theme is the redesign's design-token singleton (qt-redesign Phase 1):
# reachable from any screen as `Theme`.
set_source_files_properties(qml/theme/Theme.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE)

# ActiveJobsStore is the global active-jobs singleton
# (backlog/active-jobs-store.md): owns one JobPoller per in-flight job so
# progress survives navigation. Reachable from any screen as `ActiveJobsStore`.
set_source_files_properties(qml/ActiveJobsStore.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE)

# AudioDevicesStore is the shared capture-device catalogue singleton: one
# `GET /api/v1/audio/devices` cache for the recording screen and settings panel.
set_source_files_properties(qml/AudioDevicesStore.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE)

option(MA_ENABLE_QML_CACHEGEN
    "Compile QML into cachegen C++ files for the GUI target" ON)
set(_qml_cachegen_args)
if(NOT MA_ENABLE_QML_CACHEGEN)
    list(APPEND _qml_cachegen_args NO_CACHEGEN)
endif()

qt_add_qml_module(meeting-assistant-qt
    URI MeetingAssistant
    VERSION 1.0
    ${_qml_cachegen_args}
    QML_FILES
        qml/Main.qml
        qml/AppShell.qml
        qml/MeetingStore.qml
        qml/SettingsStore.qml
        qml/ActiveJobsStore.qml
        qml/AudioDevicesStore.qml
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
        qml/screens/settings/LanguagePanel.qml
        qml/components/PipelineProgress.qml
        qml/components/MeetyButton.qml
        qml/components/MeetyComboBox.qml
        qml/components/MeetyField.qml
        qml/components/MeetySpinBox.qml
        qml/components/MeetySwitch.qml
        qml/components/MeetySegmented.qml
        qml/components/MeetyLevelMeter.qml
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
        qml/components/AudioPlayer.qml
        qml/components/MeetyWordmark.qml
        qml/components/SettingsRow.qml
        qml/components/ProtocolDocument.qml
        qml/theme/Theme.qml
        qml/theme/ThemeGallery.qml
        qml/i18n/errors.js)

# Russian is the source language of the qsTr strings, so English is the only
# translation. The .qm is embedded as :/i18n/meeting-assistant_en.qm (Language.cpp
# loads it). Refresh the .ts with `cmake --build <dir> --target update_translations`.
qt_add_translations(meeting-assistant-qt
    TS_FILES i18n/meeting-assistant_en.ts
    RESOURCE_PREFIX "/i18n"
    LUPDATE_OPTIONS -locations none)

# Qt's own strings (the macOS application menu, standard dialogs) are English in
# Qt. A Russian run loads Qt's Russian translation, embedded here as
# :/i18n/qtbase_ru.qm (Language.cpp). It comes from the Qt install's
# qttranslations; without it those strings stay English.
function(ma_add_qt_base_translation target)
    set(_qm "${QT6_INSTALL_PREFIX}/${QT6_INSTALL_TRANSLATIONS}/qtbase_ru.qm")
    if(NOT EXISTS "${_qm}")
        message(WARNING "No ${_qm}: Qt's own strings stay English in a Russian run")
        return()
    endif()
    set_source_files_properties("${_qm}" PROPERTIES QT_RESOURCE_ALIAS "qtbase_ru.qm")
    qt_add_resources(${target} "qt_base_translation" PREFIX "/i18n" FILES "${_qm}")
endfunction()
ma_add_qt_base_translation(meeting-assistant-qt)

# Bundled design fonts (qt-redesign): variable TTFs registered as :/fonts/*,
# loaded by Theme.qml's FontLoaders. All OFL-licensed: the license must travel
# with the fonts, so the install rules below ship every file in MA_FONT_LICENSES.
# tests/serif_typeface loads the same MA_FONT_FILES.
set(MA_FONT_FILES
    fonts/Geist-Variable.ttf
    fonts/Newsreader-Variable.ttf
    fonts/Newsreader-Italic-Variable.ttf
    fonts/SourceSerif4-Variable.ttf
    fonts/SourceSerif4-Italic-Variable.ttf
    fonts/JetBrainsMono-Variable.ttf)
set(MA_FONT_LICENSES
    fonts/OFL-Geist.txt
    fonts/OFL-Newsreader.txt
    fonts/OFL-SourceSerif4.txt
    fonts/OFL-JetBrainsMono.txt)
qt_add_resources(meeting-assistant-qt "meety_fonts"
    PREFIX "/"
    FILES ${MA_FONT_FILES})

# Fusion fallback: compiled-in `:/qtquickcontrols2.conf`. Qt Quick Controls
# reads this at startup; combined with QQuickStyle::setStyle("Fusion") in
# main.cpp it makes Fusion non-overridable. We never link/ship the Material or
# Universal style plugins.
qt_add_resources(meeting-assistant-qt "controls_conf"
    PREFIX "/"
    FILES qtquickcontrols2.conf)

# Shared Qt (QT6_IS_SHARED_LIBS_BUILD): the QML engine resolves and loads its
# plugins at runtime, so the static plugin-import scan run at target
# finalization is pure noise here — it warns that style-impl plugin targets
# (Fusion/Basic/Material/Shapes/Dialogs/labs.models …) "do not exist in the
# current scope" because this distro Qt doesn't export those static targets.
# Skip the scan for a shared build; keep it for a static Qt, where those
# plugins must actually be linked into the executable.
if(QT6_IS_SHARED_LIBS_BUILD)
    set_target_properties(meeting-assistant-qt PROPERTIES
        QT_QML_MODULE_NO_IMPORT_SCAN ON)
endif()

target_link_libraries(meeting-assistant-qt PRIVATE
    Qt6::Quick
    Qt6::QuickControls2
    Qt6::Network
    Qt6::Multimedia
    Qt6::Svg)

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

# Qt-VERSION.txt, which the LGPL notices point at, must name the Qt this build
# links, and that differs per OS (see packaging/README.md), so it is generated.
set(_qt_version_notice "${CMAKE_CURRENT_BINARY_DIR}/licenses/Qt-VERSION.txt")
configure_file("${_lic_dir}/Qt-VERSION.txt.in" "${_qt_version_notice}" @ONLY)

# LGPLv3: written offer + relink notes travel inside the artefact.
function(ma_install_licenses destination)
    install(DIRECTORY "${_lic_dir}/" DESTINATION "${destination}"
        PATTERN "Qt-VERSION.txt.in" EXCLUDE)
    install(FILES "${_qt_version_notice}" DESTINATION "${destination}")
    install(FILES ${MA_FONT_LICENSES} DESTINATION "${destination}")
endfunction()

if(APPLE AND MA_MACOS_APP_BUNDLE)
    install(TARGETS meeting-assistant-qt BUNDLE DESTINATION .)
    # macdeployqt does NOT copy the helper — install it into Contents/MacOS.
    install(PROGRAMS "${MEETING_SERVER_BIN}"
        DESTINATION "MeetingAssistant.app/Contents/MacOS")
    ma_install_licenses("MeetingAssistant.app/Contents/Resources/licenses")
elseif(WIN32)
    install(TARGETS meeting-assistant-qt RUNTIME DESTINATION .)
    install(PROGRAMS "${MEETING_SERVER_BIN}" DESTINATION "."
        RENAME "${_sidecar_installed_name}")
    ma_install_licenses("licenses")
else() # Linux (AppImage AppDir / generic prefix)
    install(TARGETS meeting-assistant-qt RUNTIME DESTINATION bin)
    install(PROGRAMS "${MEETING_SERVER_BIN}" DESTINATION bin
        RENAME "${_sidecar_installed_name}")
    install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../packaging/linux/meeting-assistant.desktop"
        DESTINATION share/applications)
    ma_install_licenses(share/doc/meeting-assistant/licenses)
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
