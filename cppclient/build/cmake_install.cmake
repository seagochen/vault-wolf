# Install script for directory: /home/user/projects/vault-wolf/cppclient

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "/usr/local")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Release")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Install shared libraries without execute permission?
if(NOT DEFINED CMAKE_INSTALL_SO_NO_EXE)
  set(CMAKE_INSTALL_SO_NO_EXE "1")
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "FALSE")
endif()

# Set path to fallback-tool for dependency-resolution.
if(NOT DEFINED CMAKE_OBJDUMP)
  set(CMAKE_OBJDUMP "/usr/bin/objdump")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  foreach(file
      "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libtwsclient.so.9.79.02"
      "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libtwsclient.so.1"
      )
    if(EXISTS "${file}" AND
       NOT IS_SYMLINK "${file}")
      file(RPATH_CHECK
           FILE "${file}"
           RPATH "")
    endif()
  endforeach()
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE SHARED_LIBRARY FILES
    "/home/user/projects/vault-wolf/cppclient/build/lib/libtwsclient.so.9.79.02"
    "/home/user/projects/vault-wolf/cppclient/build/lib/libtwsclient.so.1"
    )
  foreach(file
      "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libtwsclient.so.9.79.02"
      "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/libtwsclient.so.1"
      )
    if(EXISTS "${file}" AND
       NOT IS_SYMLINK "${file}")
      if(CMAKE_INSTALL_DO_STRIP)
        execute_process(COMMAND "/usr/bin/strip" "${file}")
      endif()
    endif()
  endforeach()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE SHARED_LIBRARY FILES "/home/user/projects/vault-wolf/cppclient/build/lib/libtwsclient.so")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "/home/user/projects/vault-wolf/cppclient/build/lib/libtwsclient.a")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/twsclient" TYPE FILE FILES
    "/home/user/projects/vault-wolf/cppclient/client/CommissionAndFeesReport.h"
    "/home/user/projects/vault-wolf/cppclient/client/CommonDefs.h"
    "/home/user/projects/vault-wolf/cppclient/client/Contract.h"
    "/home/user/projects/vault-wolf/cppclient/client/ContractCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/Decimal.h"
    "/home/user/projects/vault-wolf/cppclient/client/DefaultEWrapper.h"
    "/home/user/projects/vault-wolf/cppclient/client/DepthMktDataDescription.h"
    "/home/user/projects/vault-wolf/cppclient/client/EClient.h"
    "/home/user/projects/vault-wolf/cppclient/client/EClientException.h"
    "/home/user/projects/vault-wolf/cppclient/client/EClientMsgSink.h"
    "/home/user/projects/vault-wolf/cppclient/client/EClientSocket.h"
    "/home/user/projects/vault-wolf/cppclient/client/EClientUtils.h"
    "/home/user/projects/vault-wolf/cppclient/client/EDecoder.h"
    "/home/user/projects/vault-wolf/cppclient/client/EDecoderUtils.h"
    "/home/user/projects/vault-wolf/cppclient/client/EMessage.h"
    "/home/user/projects/vault-wolf/cppclient/client/EMutex.h"
    "/home/user/projects/vault-wolf/cppclient/client/EOrderDecoder.h"
    "/home/user/projects/vault-wolf/cppclient/client/EPosixClientSocketPlatform.h"
    "/home/user/projects/vault-wolf/cppclient/client/EReader.h"
    "/home/user/projects/vault-wolf/cppclient/client/EReaderOSSignal.h"
    "/home/user/projects/vault-wolf/cppclient/client/EReaderSignal.h"
    "/home/user/projects/vault-wolf/cppclient/client/ESocket.h"
    "/home/user/projects/vault-wolf/cppclient/client/ETransport.h"
    "/home/user/projects/vault-wolf/cppclient/client/EWrapper.h"
    "/home/user/projects/vault-wolf/cppclient/client/EWrapper_prototypes.h"
    "/home/user/projects/vault-wolf/cppclient/client/Execution.h"
    "/home/user/projects/vault-wolf/cppclient/client/ExecutionCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/FamilyCode.h"
    "/home/user/projects/vault-wolf/cppclient/client/HistogramEntry.h"
    "/home/user/projects/vault-wolf/cppclient/client/HistoricalSession.h"
    "/home/user/projects/vault-wolf/cppclient/client/HistoricalTick.h"
    "/home/user/projects/vault-wolf/cppclient/client/HistoricalTickBidAsk.h"
    "/home/user/projects/vault-wolf/cppclient/client/HistoricalTickLast.h"
    "/home/user/projects/vault-wolf/cppclient/client/IExternalizable.h"
    "/home/user/projects/vault-wolf/cppclient/client/IneligibilityReason.h"
    "/home/user/projects/vault-wolf/cppclient/client/MarginCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/NewsProvider.h"
    "/home/user/projects/vault-wolf/cppclient/client/OperatorCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/Order.h"
    "/home/user/projects/vault-wolf/cppclient/client/OrderCancel.h"
    "/home/user/projects/vault-wolf/cppclient/client/OrderCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/OrderState.h"
    "/home/user/projects/vault-wolf/cppclient/client/PercentChangeCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/PriceCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/PriceIncrement.h"
    "/home/user/projects/vault-wolf/cppclient/client/ScannerSubscription.h"
    "/home/user/projects/vault-wolf/cppclient/client/SoftDollarTier.h"
    "/home/user/projects/vault-wolf/cppclient/client/StdAfx.h"
    "/home/user/projects/vault-wolf/cppclient/client/TagValue.h"
    "/home/user/projects/vault-wolf/cppclient/client/TickAttrib.h"
    "/home/user/projects/vault-wolf/cppclient/client/TickAttribBidAsk.h"
    "/home/user/projects/vault-wolf/cppclient/client/TickAttribLast.h"
    "/home/user/projects/vault-wolf/cppclient/client/TimeCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/TwsSocketClientErrors.h"
    "/home/user/projects/vault-wolf/cppclient/client/Utils.h"
    "/home/user/projects/vault-wolf/cppclient/client/VolumeCondition.h"
    "/home/user/projects/vault-wolf/cppclient/client/WshEventData.h"
    "/home/user/projects/vault-wolf/cppclient/client/bar.h"
    "/home/user/projects/vault-wolf/cppclient/client/platformspecific.h"
    "/home/user/projects/vault-wolf/cppclient/client/resource.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/twsclient/protobufUnix" TYPE FILE FILES
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/CancelOrderRequest.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/ComboLeg.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/Contract.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/DeltaNeutralContract.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/ErrorMessage.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/Execution.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/ExecutionDetails.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/ExecutionDetailsEnd.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/ExecutionFilter.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/ExecutionRequest.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/GlobalCancelRequest.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OpenOrder.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OpenOrdersEnd.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/Order.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OrderAllocation.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OrderCancel.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OrderCondition.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OrderState.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/OrderStatus.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/PlaceOrderRequest.pb.h"
    "/home/user/projects/vault-wolf/cppclient/client/protobufUnix/SoftDollarTier.pb.h"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient/twsclientTargets.cmake")
    file(DIFFERENT _cmake_export_file_changed FILES
         "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient/twsclientTargets.cmake"
         "/home/user/projects/vault-wolf/cppclient/build/CMakeFiles/Export/312a905829b265efc49276485b614972/twsclientTargets.cmake")
    if(_cmake_export_file_changed)
      file(GLOB _cmake_old_config_files "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient/twsclientTargets-*.cmake")
      if(_cmake_old_config_files)
        string(REPLACE ";" ", " _cmake_old_config_files_text "${_cmake_old_config_files}")
        message(STATUS "Old export file \"$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient/twsclientTargets.cmake\" will be replaced.  Removing files [${_cmake_old_config_files_text}].")
        unset(_cmake_old_config_files_text)
        file(REMOVE ${_cmake_old_config_files})
      endif()
      unset(_cmake_old_config_files)
    endif()
    unset(_cmake_export_file_changed)
  endif()
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient" TYPE FILE FILES "/home/user/projects/vault-wolf/cppclient/build/CMakeFiles/Export/312a905829b265efc49276485b614972/twsclientTargets.cmake")
  if(CMAKE_INSTALL_CONFIG_NAME MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient" TYPE FILE FILES "/home/user/projects/vault-wolf/cppclient/build/CMakeFiles/Export/312a905829b265efc49276485b614972/twsclientTargets-release.cmake")
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/cmake/twsclient" TYPE FILE FILES
    "/home/user/projects/vault-wolf/cppclient/build/twsclientConfig.cmake"
    "/home/user/projects/vault-wolf/cppclient/build/twsclientConfigVersion.cmake"
    )
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib/pkgconfig" TYPE FILE FILES "/home/user/projects/vault-wolf/cppclient/build/twsclient.pc")
endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
if(CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "/home/user/projects/vault-wolf/cppclient/build/install_local_manifest.txt"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
if(CMAKE_INSTALL_COMPONENT)
  if(CMAKE_INSTALL_COMPONENT MATCHES "^[a-zA-Z0-9_.+-]+$")
    set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INSTALL_COMPONENT}.txt")
  else()
    string(MD5 CMAKE_INST_COMP_HASH "${CMAKE_INSTALL_COMPONENT}")
    set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INST_COMP_HASH}.txt")
    unset(CMAKE_INST_COMP_HASH)
  endif()
else()
  set(CMAKE_INSTALL_MANIFEST "install_manifest.txt")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  file(WRITE "/home/user/projects/vault-wolf/cppclient/build/${CMAKE_INSTALL_MANIFEST}"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
endif()
