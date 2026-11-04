qt_add_executable(tst_first_run
    tst_first_run.cpp
    ../../src/AppPreferences.h ../../src/AppPreferences.cpp)

target_include_directories(tst_first_run PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/../../src")
target_compile_definitions(tst_first_run PRIVATE
    MA_FIRST_RUN_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports"
    MA_WELCOME_QML_FILE="${CMAKE_CURRENT_SOURCE_DIR}/../../qml/screens/WelcomeScreen.qml")

ma_add_test_qml_module(tst_first_run
    APP_QML
        components/MeetyButton.qml
        components/MeetyIcon.qml
        components/MeetyWordmark.qml
        theme/Theme.qml)

add_test(NAME qt_first_run_screen_states COMMAND tst_first_run)
set_tests_properties(qt_first_run_screen_states PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
