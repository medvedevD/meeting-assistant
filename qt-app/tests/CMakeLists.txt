# Offscreen Qt boot smoke (section-08). Compiles the real GUI controller
# sources (SidecarManager + ApiClient — the same files main.cpp builds, so no
# logic can drift between the shipped app and the test) plus the generated
# ClientProtocol.h, and runs them headlessly against a real meeting-server.

find_package(Qt6 6.7 REQUIRED COMPONENTS Test Gui Network)

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
