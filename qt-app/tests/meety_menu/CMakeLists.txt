qt_add_executable(tst_meety_menu
    tst_meety_menu.cpp)

target_compile_definitions(tst_meety_menu PRIVATE
    MA_MENU_QML_IMPORT_DIR="${CMAKE_CURRENT_BINARY_DIR}/imports")

ma_add_test_qml_module(tst_meety_menu
    APP_QML
        components/MeetyMenu.qml
        components/MeetyMenuItem.qml
        components/MeetyIcon.qml
        theme/Theme.qml)

add_test(NAME qt_meety_menu_long_items_expand COMMAND tst_meety_menu)
set_tests_properties(qt_meety_menu_long_items_expand PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    TIMEOUT 30)
