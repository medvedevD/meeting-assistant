qt_add_executable(tst_protocol_document
    tst_protocol_document.cpp)

target_compile_definitions(tst_protocol_document PRIVATE
    MA_PROTOCOL_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports"
    MA_PROTOCOL_QML_FILE="${CMAKE_CURRENT_SOURCE_DIR}/../../qml/components/ProtocolDocument.qml")

ma_add_test_qml_module(tst_protocol_document
    APP_QML
        components/ProtocolDocument.qml
        components/MeetyIcon.qml
        theme/Theme.qml)

add_test(NAME qt_protocol_document_markdown_blocks COMMAND tst_protocol_document)
set_tests_properties(qt_protocol_document_markdown_blocks PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
