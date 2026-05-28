# Fails the test if any QML file references the removed synchronous
# POST /api/v1/transcribe shortcut. Scans plain text — comments count too,
# which is intentional: stale doc comments mislead the next reader.
file(GLOB_RECURSE _qml_files
    LIST_DIRECTORIES false
    "${SEARCH_DIR}/*.qml"
    "${SEARCH_DIR}/*.js")

set(_offenders "")
foreach(_f IN LISTS _qml_files)
    file(READ "${_f}" _contents)
    string(FIND "${_contents}" "/api/v1/transcribe" _idx)
    # Substring match — /api/v1/transcription-models doesn't share the prefix.
    if(NOT _idx EQUAL -1)
        list(APPEND _offenders "${_f}")
    endif()
endforeach()

if(_offenders)
    message(FATAL_ERROR
        "QML must not call POST /api/v1/transcribe (route removed in the "
        "transcription-progress-visibility task — use /reprocess kind=transcribe). "
        "Offending files:\n  ${_offenders}")
endif()
