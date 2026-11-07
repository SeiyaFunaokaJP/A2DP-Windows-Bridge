# bluecaster_decode - offline checker / decoder for the media stream recorded in
# an BlueCaster debug-mode HCI capture (.pklg). Developer tool: not part of
# the release package; tools/emu uses it to check what the app sent.
#
# A project of its own (the app is built with Cargo):
#   cmake -S tools/a2dp_decode -B build/a2dp_decode
#   cmake --build build/a2dp_decode --config Release
#
# Links only codec libraries (Bluedroid SBC decoder, fdk-aac, libopenaptx,
# libopus, liblc3 for LE Audio), not BTstack. See THIRD_PARTY_LICENSES.md,
# "Developer tool: bluecaster_decode".

cmake_minimum_required(VERSION 3.16)
project(bluecaster_decode LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

set(EXTERN ${CMAKE_CURRENT_SOURCE_DIR}/../../extern)
set(BTSTACK_ROOT ${EXTERN}/btstack)
set(SBC_DECODER_DIR ${BTSTACK_ROOT}/3rd-party/bluedroid/decoder)
set(LC3_SRC_DIR ${BTSTACK_ROOT}/3rd-party/lc3-google)
set(OPENAPTX_SRC_DIR ${EXTERN}/libopenaptx)
set(FDK_AAC_DIR ${EXTERN}/fdk-aac)
set(OPUS_DIR ${EXTERN}/opus)

foreach(f ${SBC_DECODER_DIR}/srce/decoder-sbc.c ${LC3_SRC_DIR}/src/lc3.c ${OPENAPTX_SRC_DIR}/openaptx.c
        ${FDK_AAC_DIR}/CMakeLists.txt ${OPUS_DIR}/CMakeLists.txt)
    if(NOT EXISTS ${f})
        message(FATAL_ERROR "${f} not found (git submodule update --init --recursive)")
    endif()
endforeach()

if(MSVC)
    set(THIRD_PARTY_OPTIONS /W1 /utf-8)
else()
    set(THIRD_PARTY_OPTIONS -w -O2)
endif()

# SBC decoder: pure Bluedroid (Apache-2.0) from BTstack's 3rd-party folder,
# with no BTstack glue
add_library(sbc_decoder STATIC
    ${SBC_DECODER_DIR}/srce/alloc.c
    ${SBC_DECODER_DIR}/srce/bitalloc.c
    ${SBC_DECODER_DIR}/srce/bitalloc-sbc.c
    ${SBC_DECODER_DIR}/srce/bitstream-decode.c
    ${SBC_DECODER_DIR}/srce/decoder-oina.c
    ${SBC_DECODER_DIR}/srce/decoder-private.c
    ${SBC_DECODER_DIR}/srce/decoder-sbc.c
    ${SBC_DECODER_DIR}/srce/dequant.c
    ${SBC_DECODER_DIR}/srce/framing.c
    ${SBC_DECODER_DIR}/srce/framing-sbc.c
    ${SBC_DECODER_DIR}/srce/oi_codec_version.c
    ${SBC_DECODER_DIR}/srce/synthesis-8-generated.c
    ${SBC_DECODER_DIR}/srce/synthesis-dct8.c
    ${SBC_DECODER_DIR}/srce/synthesis-sbc.c
)
target_include_directories(sbc_decoder PUBLIC ${SBC_DECODER_DIR}/include)
target_compile_options(sbc_decoder PRIVATE ${THIRD_PARTY_OPTIONS})

# libopenaptx 0.2.0 (LGPL-2.1+; see THIRD_PARTY_LICENSES.md)
add_library(openaptx STATIC ${OPENAPTX_SRC_DIR}/openaptx.c)
target_include_directories(openaptx PUBLIC ${OPENAPTX_SRC_DIR})
target_compile_options(openaptx PRIVATE ${THIRD_PARTY_OPTIONS})

# liblc3 (Google, Apache-2.0) bundled in BTstack
file(GLOB LC3_SRC ${LC3_SRC_DIR}/src/*.c)
add_library(lc3 STATIC ${LC3_SRC})
target_include_directories(lc3 PUBLIC ${LC3_SRC_DIR}/include)
target_compile_options(lc3 PRIVATE ${THIRD_PARTY_OPTIONS})
if(MSVC)
    target_compile_definitions(lc3 PRIVATE _USE_MATH_DEFINES)
endif()

# fdk-aac (decoder)
set(BUILD_PROGRAMS OFF CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
add_subdirectory(${FDK_AAC_DIR} ${CMAKE_BINARY_DIR}/fdk-aac EXCLUDE_FROM_ALL)

# libopus (Xiph.Org, BSD-3-Clause): its own CMake build, static, no
# programs / tests / DNN features
set(OPUS_BUILD_PROGRAMS OFF CACHE BOOL "" FORCE)
set(OPUS_BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(OPUS_INSTALL_PKG_CONFIG_MODULE OFF CACHE BOOL "" FORCE)
set(OPUS_INSTALL_CMAKE_CONFIG_MODULE OFF CACHE BOOL "" FORCE)
add_subdirectory(${OPUS_DIR} ${CMAKE_BINARY_DIR}/opus EXCLUDE_FROM_ALL)

add_executable(bluecaster_decode a2dp_decode.cpp le_audio_decode.cpp)
target_link_libraries(bluecaster_decode PRIVATE sbc_decoder fdk-aac openaptx lc3 opus)
target_include_directories(bluecaster_decode PRIVATE
    ${FDK_AAC_DIR}/libAACdec/include
    ${FDK_AAC_DIR}/libSYS/include
)

if(MSVC)
    target_compile_options(bluecaster_decode PRIVATE /W3 /utf-8)
    target_compile_definitions(bluecaster_decode PRIVATE _CRT_SECURE_NO_WARNINGS)
else()
    target_compile_options(bluecaster_decode PRIVATE -Wall -Wextra)
endif()
