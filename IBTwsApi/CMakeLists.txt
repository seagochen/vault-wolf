# IBTwsApi/CMakeLists.txt

cmake_minimum_required(VERSION 3.5)
project(IBKR LANGUAGES CXX)

# --- 基本配置 ---
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
    message(STATUS "未指定构建类型，默认为 Debug。")
else()
    message(STATUS "构建类型为: ${CMAKE_BUILD_TYPE}")
endif()

set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# --- 输出目录设置 ---
# 注意：此处的输出目录设置已被移除，将由顶层 CMakeLists.txt 控制
# message(STATUS "库文件输出目录将由顶层 CMakeLists.txt 控制。")

# --- Ninja 构建器特定补丁 ---
if (CMAKE_GENERATOR MATCHES "Ninja")
    file(WRITE "${CMAKE_BINARY_DIR}/CLANGMakeRulesOverwrite.cmake"
        "STRING(REPLACE \"-MD\" \"-MMD\" CMAKE_DEPFILE_FLAGS_C \"${CMAKE_DEPFILE_FLAGS_C}\")\n"
        "STRING(REPLACE \"-MD\" \"-MMD\" CMAKE_DEPFILE_FLAGS_CXX \"${CMAKE_DEPFILE_FLAGS_CXX}\")\n"
    )
    set(CMAKE_USER_MAKE_RULES_OVERRIDE "${CMAKE_BINARY_DIR}/CLANGMakeRulesOverwrite.cmake" CACHE INTERNAL "")
    message(STATUS "Ninja generator detected. Applied CLANGMakeRulesOverwrite patch.")
endif ()

# --- Debug 构建特定设置 ---
if(CMAKE_BUILD_TYPE MATCHES "^[Dd][Ee][Bb][Uu][Gg]$")
    set(CMAKE_CXX_FLAGS_DEBUG_INIT "${CMAKE_CXX_FLAGS_DEBUG_INIT} -D_GLIBCXX_DEBUG")
    set(CMAKE_DEBUG_POSTFIX d) # 为Debug库名添加'd'后缀, e.g., libtwsapi_sharedd.so
    message(STATUS "Debug 构建: 添加了 _GLIBCXX_DEBUG 编译标志和 'd' 库后缀。")
else()
    set(CMAKE_DEBUG_POSTFIX "") # Release或其他模式下确保后缀为空
    message(STATUS "Release 或其他构建类型。")
endif()

# --- 查找依赖库 ---
find_package(Threads REQUIRED) # POSIX 线程库

# --- 头文件和源文件 ---
# 注意：原先的 include_directories 是全局的，现在我们将它与 target 关联
# include_directories("${CMAKE_CURRENT_SOURCE_DIR}/include") # 改为 target_include_directories

# 收集源文件
file(GLOB_RECURSE ALL_CLIENT_SOURCE_FILES
    "${CMAKE_CURRENT_SOURCE_DIR}/client/*.cpp"
)
if(NOT ALL_CLIENT_SOURCE_FILES)
    message(WARNING "在 ${CMAKE_CURRENT_SOURCE_DIR}/client/ 目录中未找到任何 .cpp 源文件。")
else()
    message(STATUS "找到的源文件: ${ALL_CLIENT_SOURCE_FILES}")
endif()

# --- 构建库 ---
if(ALL_CLIENT_SOURCE_FILES)
    # 静态库
    add_library(twsapi_static STATIC ${ALL_CLIENT_SOURCE_FILES})
    target_link_libraries(twsapi_static PUBLIC Threads::Threads)
    set_target_properties(twsapi_static PROPERTIES POSITION_INDEPENDENT_CODE ON)
    target_include_directories(twsapi_static
        PUBLIC
            "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>"
            "$<INSTALL_INTERFACE:include>"
    )
    # 为 twsapi_static 也链接 bid_imported，因为它使用相同的源文件
    if(TARGET bid_imported)
        target_link_libraries(twsapi_static PUBLIC bid_imported) # 或者 PRIVATE
        message(STATUS "IBTwsApi: twsapi_static 已链接到 bid_imported。")
    else()
        message(WARNING "IBTwsApi: 目标 bid_imported 未找到，twsapi_static 可能链接失败。")
    endif()
    message(STATUS "已配置静态库 target: twsapi_static")


    # 共享库
    add_library(twsapi_shared SHARED ${ALL_CLIENT_SOURCE_FILES})
    # 首先链接已有的库 (如 Threads)
    target_link_libraries(twsapi_shared PUBLIC Threads::Threads)
    set_target_properties(twsapi_shared PROPERTIES POSITION_INDEPENDENT_CODE ON)
    target_include_directories(twsapi_shared
        PUBLIC
            "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>"
            "$<INSTALL_INTERFACE:include>"
    )

    # 然后，添加链接 bid_imported 的逻辑
    # ================================================================ #
    #                 ↓↓↓ 将代码块添加到这里 ↓↓↓                   #
    # ================================================================ #
    if(TARGET bid_imported)
        target_link_libraries(twsapi_shared PUBLIC bid_imported) # 将 bid_imported 添加到链接列表
        message(STATUS "IBTwsApi: twsapi_shared 已成功链接到 bid_imported。")
    else()
        message(WARNING "IBTwsApi: 目标 bid_imported 未找到，twsapi_shared 可能链接失败。")
    endif()
    # ================================================================ #
    #                 ↑↑↑ 新增代码块结束 ↑↑↑                     #
    # ================================================================ #

    message(STATUS "已配置共享库 target: twsapi_shared")

else()
    message(WARNING "由于未找到源文件，无法创建 twsapi_static 和 twsapi_shared 库。")
endif()

# --- 可选：代码格式化工具 ---
option(ENABLE_FORMATTER "启用 clang-format 代码格式化目标" OFF)
if(ENABLE_FORMATTER)
    find_program(CLANG_FORMAT_EXE NAMES clang-format-10 clang-format)
    if(CLANG_FORMAT_EXE)
        file(GLOB_RECURSE ALL_PROJECT_FORMAT_FILES
            "${CMAKE_CURRENT_SOURCE_DIR}/client/*.cpp"
            "${CMAKE_CURRENT_SOURCE_DIR}/include/*.h"
        )
        if(ALL_PROJECT_FORMAT_FILES)
            add_custom_target(format
                COMMAND ${CLANG_FORMAT_EXE} -style=file -i ${ALL_PROJECT_FORMAT_FILES}
                COMMENT "使用 clang-format 格式化 C++/Header 文件"
            )
            message(STATUS "已添加 'format' 目标 (使用 clang-format)。")
        else()
            message(WARNING "未找到用于格式化的源文件或头文件，'format' 目标虽然创建但可能无效。")
        endif()
    else()
        message(WARNING "未找到 clang-format。'format' 目标将不可用。")
    endif()
endif()

message(STATUS "IBTwsApi CMake 配置完成。")