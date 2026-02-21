#----------------------------------------------------------------
# Generated CMake target import file for configuration "Release".
#----------------------------------------------------------------

# Commands may need to know the format version.
set(CMAKE_IMPORT_FILE_VERSION 1)

# Import target "twsclient::twsclient_shared" for configuration "Release"
set_property(TARGET twsclient::twsclient_shared APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(twsclient::twsclient_shared PROPERTIES
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib/libtwsclient.so.9.79.02"
  IMPORTED_SONAME_RELEASE "libtwsclient.so.1"
  )

list(APPEND _cmake_import_check_targets twsclient::twsclient_shared )
list(APPEND _cmake_import_check_files_for_twsclient::twsclient_shared "${_IMPORT_PREFIX}/lib/libtwsclient.so.9.79.02" )

# Import target "twsclient::twsclient_static" for configuration "Release"
set_property(TARGET twsclient::twsclient_static APPEND PROPERTY IMPORTED_CONFIGURATIONS RELEASE)
set_target_properties(twsclient::twsclient_static PROPERTIES
  IMPORTED_LINK_INTERFACE_LANGUAGES_RELEASE "C;CXX"
  IMPORTED_LOCATION_RELEASE "${_IMPORT_PREFIX}/lib/libtwsclient.a"
  )

list(APPEND _cmake_import_check_targets twsclient::twsclient_static )
list(APPEND _cmake_import_check_files_for_twsclient::twsclient_static "${_IMPORT_PREFIX}/lib/libtwsclient.a" )

# Commands beyond this point should not need to know the version.
set(CMAKE_IMPORT_FILE_VERSION)
