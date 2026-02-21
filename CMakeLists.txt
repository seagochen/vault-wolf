# VaultWolf/CMakeLists.txt

cmake_minimum_required(VERSION 3.10) # 建议使用稍高版本
project(VaultWolf LANGUAGES CXX)

# --- 基本配置 ---
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Debug")
    message(STATUS "未指定构建类型，默认为 Debug。")
else()
    message(STATUS "构建类型为: ${CMAKE_BUILD_TYPE}")
endif()

set(CMAKE_CXX_STANDARD 17) # 您可以根据需要设置 C++ 标准
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# --- 输出目录设置 (对于整个项目) ---
# 这些设置将影响所有目标，包括子项目中的目标，除非子项目覆盖它们
# CMAKE_BINARY_DIR 指向构建目录 (例如 VaultWolf/build)
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib") # for static libraries (.a)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/lib") # for shared libraries (.so)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin") # for executables

message(STATUS "全局库输出目录: ${CMAKE_LIBRARY_OUTPUT_DIRECTORY}")
message(STATUS "全局可执行文件输出目录: ${CMAKE_RUNTIME_OUTPUT_DIRECTORY}")

# --- 构建外部依赖 libbid ---
set(LIBBID_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/thirds/IntelRDFPMathLib20U2")
set(LIBBID_MAKEFILE_PATH "${LIBBID_SOURCE_DIR}/makefile") # makefile 小写
# 确保您的 Makefile 在 thirds/IntelRDFPMathLib20U2/LIBRARY/ 中的确是这个名字，
# 并且已经被修改为生成 libbid.so (如之前的步骤所示)

# libbid.so 将会被编译到 LIBBID_SOURCE_DIR，然后我们将其复制到 CMake 的库输出目录
set(LIBBID_BUILT_FILE "${LIBBID_SOURCE_DIR}/libbid.so") # make 命令的直接输出
set(LIBBID_FINAL_LOCATION "${CMAKE_LIBRARY_OUTPUT_DIRECTORY}/libbid.so") # 我们希望它最终在的位置

# 创建一个自定义目标来构建 libbid
# ALL 关键字确保当您运行默认构建命令 (如 make) 时，这个目标也会被构建
add_custom_target(libbid_external_build ALL
    # 命令1: 执行 make 来编译 libbid.so
    COMMAND ${CMAKE_COMMAND} -E env CC=gcc make -C "${LIBBID_SOURCE_DIR}" CALL_BY_REF=0 GLOBAL_RND=0 GLOBAL_FLAGS=0 UNCHANGED_BINARY_FLAGS=0
    # 命令2: 清理 libbid 目录下的 .o 文件
    COMMAND find "${LIBBID_SOURCE_DIR}" -maxdepth 1 -name "*.o" -delete
    # 命令3: 将编译好的 libbid.so 复制到 CMake 的库输出目录
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${LIBBID_BUILT_FILE}" "${LIBBID_FINAL_LOCATION}"
    WORKING_DIRECTORY "${LIBBID_SOURCE_DIR}" # 指定 make 命令的工作目录
    DEPENDS "${LIBBID_MAKEFILE_PATH}" # 如果 Makefile 变了，重新构建 (可以添加更多依赖)
                                      # 例如，所有 libbid 的源文件
    BYPRODUCTS "${LIBBID_FINAL_LOCATION}" # 声明此目标最终产生的文件
    COMMENT "Building external library libbid.so"
    VERBATIM # 确保命令中的特殊字符被正确处理
)

# 创建一个 IMPORTED 库目标来代表 libbid.so
# GLOBAL 关键字使其在子目录中也可见 (例如在 IBTwsApi 中)
add_library(bid_imported SHARED IMPORTED GLOBAL)
set_property(TARGET bid_imported PROPERTY IMPORTED_LOCATION "${LIBBID_FINAL_LOCATION}")

# 确保 bid_imported 目标依赖于 libbid_external_build 目标
# 这样在尝试链接 bid_imported 之前，会先执行 libbid_external_build
add_dependencies(bid_imported libbid_external_build)

# --- 添加子项目 IBTwsApi ---
# 这会执行 IBTwsApi/CMakeLists.txt
# IBTwsApi 生成的库 (twsapi_static, twsapi_shared) 现在可用于此项目
add_subdirectory(IBTwsApi)

# --- VaultWolf 主项目代码 ---
# 添加主项目的头文件目录
# include_directories(include) # 旧方法，推荐使用 target_include_directories

# 收集主项目的源文件
file(GLOB_RECURSE VAULTWOLF_SOURCES
    "src/*.cpp"
    # 如果有 main.cpp 直接在 src 目录下，可以这样写，或者明确列出
    # "src/main.cpp"
)

if(NOT VAULTWOLF_SOURCES)
    message(WARNING "在 ${PROJECT_SOURCE_DIR}/src/ 目录中未找到任何 .cpp 源文件。主程序可能无法构建。")
    # 可以创建一个虚拟的 main.cpp 以便项目至少能配置
    file(WRITE "${PROJECT_SOURCE_DIR}/src/main.cpp" "int main() { return 0; }\n")
    set(VAULTWOLF_SOURCES "${PROJECT_SOURCE_DIR}/src/main.cpp")
    message(STATUS "创建了一个临时的 src/main.cpp 文件。")
endif()

# 添加可执行文件目标
add_executable(VaultWolfApp ${VAULTWOLF_SOURCES})

# 为 VaultWolfApp 添加它自己的头文件目录
target_include_directories(VaultWolfApp
    PRIVATE
        "${PROJECT_SOURCE_DIR}/include" # VaultWolf/include
        # IBTwsApi 的头文件目录会通过 target_link_libraries 自动继承，
        # 因为我们在 IBTwsApi/CMakeLists.txt 中为 twsapi_shared 设置了 PUBLIC include 目录
)

# 链接 twsapi_shared 到 bid_imported
# 由于 bid_imported 被定义为 GLOBAL IMPORTED, 这里可以直接使用
# if(TARGET bid_imported)
#     target_link_libraries(twsapi_shared PUBLIC bid_imported) # 或者 PRIVATE，取决于是否要传递链接
#     message(STATUS "twsapi_shared 已链接到 bid_imported。")
# else()
#     message(WARNING "目标 bid_imported 未找到，twsapi_shared 可能链接失败。")
# endif()

# 链接 VaultWolfApp 到 IBTwsApi 的共享库
# CMake 会自动处理 Debug/Release 后缀 (例如 libtwsapi_sharedd.so 或 libtwsapi_shared.so)
if(TARGET twsapi_shared)
    target_link_libraries(VaultWolfApp PRIVATE twsapi_shared)
    message(STATUS "VaultWolfApp 已链接到 twsapi_shared。")
else()
    message(WARNING "目标 twsapi_shared 未找到，VaultWolfApp 可能链接失败。")
endif()


# 如果您也想链接静态库（通常不与共享库同时链接同一个库）
# if(TARGET twsapi_static)
# target_link_libraries(VaultWolfApp PRIVATE twsapi_static)
# message(STATUS "VaultWolfApp 已链接到 twsapi_static。")
# endif()


message(STATUS "VaultWolf CMake 配置完成。")