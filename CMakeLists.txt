cmake_minimum_required(VERSION 3.16)
project(a2dpwb VERSION 1.0.0 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

# Options
option(LDAC_SOFT_FLOAT "Use software floating point for libldac" OFF)

# --- libldac encoder library ---
# Build libldac from AOSP source in extern/libldac/
set(LDAC_SRC_DIR ${CMAKE_SOURCE_DIR}/extern/libldac)

if(EXISTS ${LDAC_SRC_DIR}/src/ldaclib.c)
    # Only compile the two top-level files per AOSP Android.bp.
    # Other .c files (tables, quantization, etc.) are #included internally
    # via DECLFUNC (static) linkage — compiling them separately causes
    # undefined symbol errors.
    add_library(ldac STATIC
        ${LDAC_SRC_DIR}/src/ldaclib.c
        ${LDAC_SRC_DIR}/src/ldacBT.c
    )
    target_include_directories(ldac PUBLIC
        ${LDAC_SRC_DIR}/inc
    )
    target_include_directories(ldac PRIVATE
        ${LDAC_SRC_DIR}/src
    )

    # ABR (Adaptive Bit Rate) support
    if(EXISTS ${LDAC_SRC_DIR}/abr/src/ldacBT_abr.c)
        target_sources(ldac PRIVATE ${LDAC_SRC_DIR}/abr/src/ldacBT_abr.c)
        target_include_directories(ldac PUBLIC ${LDAC_SRC_DIR}/abr/inc)
    endif()
    target_compile_definitions(ldac PRIVATE
        $<$<BOOL:${LDAC_SOFT_FLOAT}>:_LDAC_SOFT_FLOAT_>
    )
    if(MSVC)
        # Provide unistd.h shim for AOSP code that includes it
        target_include_directories(ldac PRIVATE ${CMAKE_SOURCE_DIR}/compat/msvc)
        # Force-include compatibility header to handle GCC __attribute__ etc.
        target_compile_options(ldac PRIVATE /W3
            /FI${CMAKE_SOURCE_DIR}/compat/msvc/ldac_compat.h)
    else()
        target_compile_options(ldac PRIVATE -Wall -Wextra -O2)
    endif()
    message(STATUS "libldac: Found and configured")
else()
    message(WARNING "libldac not found at ${LDAC_SRC_DIR}. "
        "Run: git submodule update --init --recursive")
endif()

# --- libopenaptx (aptX / aptX HD encoder) ---
set(OPENAPTX_SRC_DIR ${CMAKE_SOURCE_DIR}/extern/libopenaptx)

if(EXISTS ${OPENAPTX_SRC_DIR}/openaptx.c)
    add_library(openaptx STATIC
        ${OPENAPTX_SRC_DIR}/openaptx.c
    )
    target_include_directories(openaptx PUBLIC ${OPENAPTX_SRC_DIR})
    if(MSVC)
        target_compile_options(openaptx PRIVATE /W3)
    else()
        target_compile_options(openaptx PRIVATE -Wall -Wextra -O2)
    endif()
    message(STATUS "libopenaptx: Found and configured")
else()
    message(WARNING "libopenaptx not found at ${OPENAPTX_SRC_DIR}. "
        "Run: git submodule add https://github.com/pali/libopenaptx.git extern/libopenaptx")
endif()

# --- BTstack Bluetooth stack (user-mode HCI/L2CAP/AVDTP via WinUSB) ---
set(BTSTACK_ROOT ${CMAKE_SOURCE_DIR}/extern/btstack)

if(EXISTS ${BTSTACK_ROOT}/src/hci.c)
    # Core sources
    file(GLOB BTSTACK_CORE_SRC ${BTSTACK_ROOT}/src/*.c)
    # Remove optional codec/audio sources that require external dependencies
    file(GLOB BTSTACK_EXCLUDE_SRC
        ${BTSTACK_ROOT}/src/btstack_lc3*.c
        ${BTSTACK_ROOT}/src/btstack_audio_generator.c
        ${BTSTACK_ROOT}/src/btstack_audio_player.c
        ${BTSTACK_ROOT}/src/btstack_audio_song.c
    )
    if(BTSTACK_EXCLUDE_SRC)
        list(REMOVE_ITEM BTSTACK_CORE_SRC ${BTSTACK_EXCLUDE_SRC})
    endif()
    # Classic BT sources (A2DP, AVDTP, SDP, etc.)
    file(GLOB BTSTACK_CLASSIC_SRC ${BTSTACK_ROOT}/src/classic/*.c)
    # Windows platform sources (run loop, WinUSB HCI transport, TLV storage)
    file(GLOB BTSTACK_WINDOWS_SRC ${BTSTACK_ROOT}/platform/windows/*.c)
    # 3rd-party dependencies
    set(BTSTACK_3RDPARTY_SRC
        ${BTSTACK_ROOT}/3rd-party/rijndael/rijndael.c
        ${BTSTACK_ROOT}/3rd-party/micro-ecc/uECC.c
        ${BTSTACK_ROOT}/3rd-party/md5/md5.c
        ${BTSTACK_ROOT}/3rd-party/yxml/yxml.c
    )

    # Chipset drivers (Realtek for TP-Link UB500 etc.)
    set(BTSTACK_CHIPSET_SRC
        ${BTSTACK_ROOT}/chipset/realtek/btstack_chipset_realtek.c
    )

    add_library(btstack STATIC
        ${BTSTACK_CORE_SRC}
        ${BTSTACK_CLASSIC_SRC}
        ${BTSTACK_WINDOWS_SRC}
        ${BTSTACK_3RDPARTY_SRC}
        ${BTSTACK_CHIPSET_SRC}
    )
    target_include_directories(btstack PUBLIC
        ${BTSTACK_ROOT}/src
        ${BTSTACK_ROOT}/platform/windows
        ${BTSTACK_ROOT}/platform/posix
        ${BTSTACK_ROOT}/chipset/realtek
        ${BTSTACK_ROOT}/3rd-party/rijndael
        ${BTSTACK_ROOT}/3rd-party/micro-ecc
        ${BTSTACK_ROOT}/3rd-party/md5
        ${BTSTACK_ROOT}/3rd-party/yxml
        ${BTSTACK_ROOT}/3rd-party/bluedroid/encoder/include
        ${BTSTACK_ROOT}/3rd-party/bluedroid/decoder/include
    )
    # btstack_config.h is in app/src/ — included via the app target
    target_include_directories(btstack PUBLIC
        ${CMAKE_SOURCE_DIR}/app/src
    )
    target_compile_definitions(btstack PRIVATE
        _WIN32_WINNT=0x0A00
        NOMINMAX
        WIN32_LEAN_AND_MEAN
    )
    if(MSVC)
        # Suppress warnings in third-party code
        target_compile_options(btstack PRIVATE /W1 /utf-8)
    else()
        target_compile_options(btstack PRIVATE -w)
    endif()
    target_link_libraries(btstack PUBLIC setupapi winusb ws2_32)
    message(STATUS "BTstack: Found and configured (WinUSB transport)")
else()
    message(WARNING "BTstack not found at ${BTSTACK_ROOT}. "
        "Run: git submodule update --init --recursive")
endif()

# --- SBC codec (encoder + decoder from BTstack's bundled Bluedroid) ---
set(SBC_ENCODER_DIR ${BTSTACK_ROOT}/3rd-party/bluedroid/encoder)
set(SBC_DECODER_DIR ${BTSTACK_ROOT}/3rd-party/bluedroid/decoder)
if(EXISTS ${SBC_ENCODER_DIR}/srce/sbc_encoder.c)
    add_library(sbc_codec STATIC
        ${SBC_ENCODER_DIR}/srce/sbc_analysis.c
        ${SBC_ENCODER_DIR}/srce/sbc_dct.c
        ${SBC_ENCODER_DIR}/srce/sbc_dct_coeffs.c
        ${SBC_ENCODER_DIR}/srce/sbc_enc_bit_alloc_mono.c
        ${SBC_ENCODER_DIR}/srce/sbc_enc_bit_alloc_ste.c
        ${SBC_ENCODER_DIR}/srce/sbc_enc_coeffs.c
        ${SBC_ENCODER_DIR}/srce/sbc_encoder.c
        ${SBC_ENCODER_DIR}/srce/sbc_packing.c
        ${BTSTACK_ROOT}/src/classic/btstack_sbc_encoder_bluedroid.c
        # Decoder (required by btstack_sbc_bluedroid.c in btstack lib)
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
    target_include_directories(sbc_codec PUBLIC
        ${SBC_ENCODER_DIR}/include
        ${SBC_DECODER_DIR}/include
        ${BTSTACK_ROOT}/src
        ${BTSTACK_ROOT}/src/classic
        ${CMAKE_SOURCE_DIR}/app/src
    )
    if(MSVC)
        target_compile_options(sbc_codec PRIVATE /W1 /utf-8)
    else()
        target_compile_options(sbc_codec PRIVATE -w)
    endif()
    message(STATUS "SBC encoder: Found and configured (Bluedroid)")
else()
    message(WARNING "SBC encoder sources not found at ${SBC_ENCODER_DIR}")
endif()

# --- fdk-aac (AAC-LC encoder for A2DP) ---
set(FDK_AAC_DIR ${CMAKE_SOURCE_DIR}/extern/fdk-aac)
if(EXISTS ${FDK_AAC_DIR}/CMakeLists.txt)
    set(BUILD_PROGRAMS OFF CACHE BOOL "" FORCE)
    set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
    add_subdirectory(${FDK_AAC_DIR} ${CMAKE_BINARY_DIR}/fdk-aac EXCLUDE_FROM_ALL)
    message(STATUS "fdk-aac: Found and configured")
else()
    message(STATUS "fdk-aac not found at ${FDK_AAC_DIR}. AAC codec will be disabled. "
        "To enable: git submodule add https://github.com/mstorsjo/fdk-aac.git extern/fdk-aac")
endif()

# --- User-mode application ---
add_subdirectory(app)
