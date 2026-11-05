qt_add_executable(tst_serif_typeface
    tst_serif_typeface.cpp)

target_compile_definitions(tst_serif_typeface PRIVATE
    MA_SERIF_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports"
    MA_SERIF_QML_FILE="${CMAKE_CURRENT_SOURCE_DIR}/../../qml/components/ProtocolDocument.qml")

ma_add_test_qml_module(tst_serif_typeface
    APP_QML
        components/ProtocolDocument.qml
        components/MeetyIcon.qml
        theme/Theme.qml)

# The app's real fonts, under the same :/fonts/ paths: the test checks which font
# each letter is drawn with, which depends on what the files actually contain.
get_filename_component(_app_dir "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
list(TRANSFORM MA_FONT_FILES PREPEND "${_app_dir}/" OUTPUT_VARIABLE _fonts)
qt_add_resources(tst_serif_typeface "test_fonts"
    PREFIX "/"
    BASE "${_app_dir}"
    FILES ${_fonts})

add_test(NAME qt_serif_typeface COMMAND tst_serif_typeface)
set_tests_properties(qt_serif_typeface PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
