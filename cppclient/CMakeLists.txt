cmake_minimum_required(VERSION 3.16)

project(twsclient
    VERSION 9.79.02
    DESCRIPTION "Interactive Brokers TWS API C++ Client Library"
    LANGUAGES C CXX
)

# ---------------------------------------------------------------------------
# Global settings
# ---------------------------------------------------------------------------
set(CMAKE_C_STANDARD   11)
set(CMAKE_CXX_STANDARD 14)
set(CMAKE_C_STANDARD_REQUIRED   ON)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Required for both .so and .a to be usable in shared contexts
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# ---------------------------------------------------------------------------
# Source collection
# ---------------------------------------------------------------------------
set(CLIENT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/client")
set(PROTO_DIR  "${CLIENT_DIR}/protobufUnix")

file(GLOB CLIENT_SOURCES  "${CLIENT_DIR}/*.cpp")
file(GLOB CLIENT_HEADERS  "${CLIENT_DIR}/*.h")
file(GLOB PROTO_SOURCES   "${PROTO_DIR}/*.cc")
file(GLOB PROTO_HEADERS   "${PROTO_DIR}/*.pb.h")

# ---------------------------------------------------------------------------
# Dependency: Google Protocol Buffers
# ---------------------------------------------------------------------------
find_package(Protobuf QUIET)

if(Protobuf_FOUND)
    message(STATUS "Protobuf ${Protobuf_VERSION} found via find_package")
    set(HAVE_PROTOBUF TRUE)
    set(PROTOBUF_LINK_TARGET protobuf::libprotobuf)
else()
    find_package(PkgConfig QUIET)
    if(PkgConfig_FOUND)
        pkg_check_modules(PROTOBUF QUIET protobuf)
    endif()
    if(PROTOBUF_FOUND)
        message(STATUS "Protobuf found via pkg-config")
        set(HAVE_PROTOBUF TRUE)
        set(PROTOBUF_LINK_TARGET ${PROTOBUF_LIBRARIES})
    else()
        message(WARNING
            "Google Protobuf not found.\n"
            "  Install it with:  sudo apt install libprotobuf-dev   (Debian/Ubuntu)\n"
            "                    sudo dnf install protobuf-devel      (Fedora/RHEL)\n"
            "  Or build from source: https://github.com/protocolbuffers/protobuf\n"
            "The protobufUnix sources will be excluded from the build."
        )
        set(HAVE_PROTOBUF FALSE)
    endif()
endif()

# ---------------------------------------------------------------------------
# Dependency: Intel BID64 Decimal Floating Point Library (libbid)
#
# Search order:
#   1. client/lib/          (developer drops libbid.a/.so here)
#   2. Standard system paths
#   3. Software stub         (bid64_stub.c — always correct for trading values)
# ---------------------------------------------------------------------------
option(USE_LIBBID_STUB
    "Always use the software BID64 stub instead of the Intel library" OFF)

if(NOT USE_LIBBID_STUB)
    find_library(LIBBID_LIBRARY
        NAMES libbid.a libbid.so bid
        PATHS
            "${CLIENT_DIR}/lib"
            "${CMAKE_CURRENT_SOURCE_DIR}/lib"
            /usr/local/lib
            /usr/lib
            /usr/lib/x86_64-linux-gnu
        NO_DEFAULT_PATH
    )
    if(NOT LIBBID_LIBRARY)
        find_library(LIBBID_LIBRARY NAMES bid libbid)
    endif()
endif()

if(LIBBID_LIBRARY AND NOT USE_LIBBID_STUB)
    message(STATUS "Intel libbid found: ${LIBBID_LIBRARY}")
    set(DECIMAL_SOURCES "")
    set(DECIMAL_LINK_TARGET "${LIBBID_LIBRARY}")
else()
    if(NOT USE_LIBBID_STUB)
        message(STATUS
            "Intel libbid not found — using built-in software BID64 stub.\n"
            "  To use the Intel library instead:\n"
            "    1. Follow instructions in cppclient/Intel_lib_build.txt\n"
            "    2. Copy libbid.a (or libbid.so) to cppclient/client/lib/\n"
            "    3. Re-run cmake"
        )
    else()
        message(STATUS "Using software BID64 stub (USE_LIBBID_STUB=ON)")
    endif()
    set(DECIMAL_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/bid64_stub.c")
    set(DECIMAL_LINK_TARGET "m")   # need -lm for pow(), log10(), etc.
endif()

# ---------------------------------------------------------------------------
# Build the combined source list
# ---------------------------------------------------------------------------
set(ALL_SOURCES ${CLIENT_SOURCES} ${DECIMAL_SOURCES})
if(HAVE_PROTOBUF)
    list(APPEND ALL_SOURCES ${PROTO_SOURCES})
endif()

set(ALL_INCLUDE_DIRS
    "$<BUILD_INTERFACE:${CLIENT_DIR}>"
    "$<INSTALL_INTERFACE:include/twsclient>"
)
if(HAVE_PROTOBUF)
    list(APPEND ALL_INCLUDE_DIRS
        "$<BUILD_INTERFACE:${PROTO_DIR}>"
        "$<INSTALL_INTERFACE:include/twsclient/protobufUnix>"
    )
    if(PROTOBUF_INCLUDE_DIRS)
        list(APPEND ALL_INCLUDE_DIRS ${PROTOBUF_INCLUDE_DIRS})
    endif()
endif()

# ---------------------------------------------------------------------------
# Shared library  (.so)
# ---------------------------------------------------------------------------
add_library(twsclient_shared SHARED ${ALL_SOURCES})

set_target_properties(twsclient_shared PROPERTIES
    OUTPUT_NAME  "twsclient"
    VERSION      "${PROJECT_VERSION}"
    SOVERSION    "1"
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib"
)

target_include_directories(twsclient_shared PUBLIC ${ALL_INCLUDE_DIRS})

target_compile_options(twsclient_shared PRIVATE
    $<$<COMPILE_LANGUAGE:CXX>:
        -Wall -Wextra -Wno-unused-parameter -Wno-switch -Wpedantic>
    $<$<COMPILE_LANGUAGE:C>:
        -Wall -Wextra -Wno-unused-parameter>
)

target_link_libraries(twsclient_shared
    PUBLIC  Threads::Threads
    PRIVATE ${DECIMAL_LINK_TARGET}
)
if(HAVE_PROTOBUF)
    # Detect whether a shared (PIC) libprotobuf is available.
    # The find_package approach returns protobuf::libprotobuf which may point
    # at a static-only build that lacks PIC code.  We probe explicitly.
    find_library(_PROTOBUF_SO NAMES protobuf PATHS ${Protobuf_LIBRARY_DIRS} NO_DEFAULT_PATH)
    if(NOT _PROTOBUF_SO)
        find_library(_PROTOBUF_SO NAMES protobuf)
    endif()
    if(_PROTOBUF_SO MATCHES "\\.so" OR _PROTOBUF_SO MATCHES "\\.dylib")
        # Shared protobuf available — link normally
        target_link_libraries(twsclient_shared PUBLIC ${PROTOBUF_LINK_TARGET})
    else()
        # Only a non-PIC static archive is present.
        # Leave protobuf symbols undefined in the .so; consumers must supply them.
        message(STATUS "Shared libprotobuf.so not found — protobuf symbols will be unresolved in the .so (consumers must link -lprotobuf)")
        target_link_options(twsclient_shared PRIVATE -Wl,--allow-shlib-undefined)
        # Still expose the include directories and link requirement to consumers
        target_link_libraries(twsclient_shared INTERFACE ${PROTOBUF_LINK_TARGET})
    endif()
endif()

# ---------------------------------------------------------------------------
# Static library  (.a)
# ---------------------------------------------------------------------------
add_library(twsclient_static STATIC ${ALL_SOURCES})

set_target_properties(twsclient_static PROPERTIES
    OUTPUT_NAME "twsclient"
    ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib"
)

target_include_directories(twsclient_static PUBLIC ${ALL_INCLUDE_DIRS})

target_compile_options(twsclient_static PRIVATE
    $<$<COMPILE_LANGUAGE:CXX>:
        -Wall -Wextra -Wno-unused-parameter -Wno-switch -Wpedantic>
    $<$<COMPILE_LANGUAGE:C>:
        -Wall -Wextra -Wno-unused-parameter>
)

target_link_libraries(twsclient_static
    PUBLIC  Threads::Threads
    PRIVATE ${DECIMAL_LINK_TARGET}
)
if(HAVE_PROTOBUF)
    target_link_libraries(twsclient_static PUBLIC ${PROTOBUF_LINK_TARGET})
endif()

# ---------------------------------------------------------------------------
# Thread support (pthreads on Linux/macOS)
# ---------------------------------------------------------------------------
find_package(Threads REQUIRED)

# ---------------------------------------------------------------------------
# Install rules
# ---------------------------------------------------------------------------
include(GNUInstallDirs)

install(
    TARGETS twsclient_shared twsclient_static
    EXPORT  twsclient-targets
    LIBRARY  DESTINATION "${CMAKE_INSTALL_LIBDIR}"
    ARCHIVE  DESTINATION "${CMAKE_INSTALL_LIBDIR}"
)

install(
    FILES ${CLIENT_HEADERS}
    DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/twsclient"
)

if(HAVE_PROTOBUF)
    install(
        FILES ${PROTO_HEADERS}
        DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/twsclient/protobufUnix"
    )
endif()

# CMake package config so downstream projects can do:
#   find_package(twsclient REQUIRED)
install(
    EXPORT twsclient-targets
    FILE       twsclientTargets.cmake
    NAMESPACE  twsclient::
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/twsclient"
)

include(CMakePackageConfigHelpers)

configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/twsclientConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/twsclientConfig.cmake"
    INSTALL_DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/twsclient"
)

write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/twsclientConfigVersion.cmake"
    VERSION      "${PROJECT_VERSION}"
    COMPATIBILITY AnyNewerVersion
)

install(
    FILES
        "${CMAKE_CURRENT_BINARY_DIR}/twsclientConfig.cmake"
        "${CMAKE_CURRENT_BINARY_DIR}/twsclientConfigVersion.cmake"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/twsclient"
)

# ---------------------------------------------------------------------------
# pkg-config file  (for Rust build.rs or any pkg-config consumer)
# ---------------------------------------------------------------------------
configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/twsclient.pc.in"
    "${CMAKE_CURRENT_BINARY_DIR}/twsclient.pc"
    @ONLY
)

install(
    FILES "${CMAKE_CURRENT_BINARY_DIR}/twsclient.pc"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/pkgconfig"
)

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
message(STATUS "")
message(STATUS "=== twsclient build summary ===")
message(STATUS "  Version       : ${PROJECT_VERSION}")
message(STATUS "  Build type    : ${CMAKE_BUILD_TYPE}")
message(STATUS "  Protobuf      : ${HAVE_PROTOBUF}")
message(STATUS "  Intel libbid  : ${LIBBID_LIBRARY}")
message(STATUS "  BID64 stub    : ${DECIMAL_SOURCES}")
message(STATUS "  Install prefix: ${CMAKE_INSTALL_PREFIX}")
message(STATUS "")
