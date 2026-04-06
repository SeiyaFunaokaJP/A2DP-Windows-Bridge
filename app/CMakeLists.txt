# A2DPWB (A2DP Windows Bridge)
# Captures system audio via WASAPI, encodes with LDAC/aptX HD/aptX LL/SBC/AAC,
# and streams over Bluetooth via BTstack (WinUSB).
# GUI frontend via wxWidgets.

# --- wxWidgets (FetchContent) ---
include(FetchContent)
FetchContent_Declare(
    wxWidgets
    GIT_REPOSITORY https://github.com/wxWidgets/wxWidgets.git
    GIT_TAG        v3.2.6
    GIT_SHALLOW    TRUE
)
set(wxBUILD_SHARED OFF CACHE BOOL "" FORCE)
set(wxUSE_WEBVIEW OFF CACHE BOOL "" FORCE)
set(wxUSE_STC OFF CACHE BOOL "" FORCE)
set(wxUSE_RIBBON OFF CACHE BOOL "" FORCE)
set(wxUSE_PROPGRID OFF CACHE BOOL "" FORCE)
set(wxUSE_AUI OFF CACHE BOOL "" FORCE)
set(wxUSE_RICHTEXT OFF CACHE BOOL "" FORCE)
set(wxUSE_MEDIACTRL OFF CACHE BOOL "" FORCE)
set(wxUSE_XRC OFF CACHE BOOL "" FORCE)
set(wxUSE_HTML OFF CACHE BOOL "" FORCE)
set(wxUSE_HELP OFF CACHE BOOL "" FORCE)
set(wxUSE_WXHTML_HELP OFF CACHE BOOL "" FORCE)
set(wxBUILD_COMPATIBILITY "3.0" CACHE STRING "" FORCE)
FetchContent_MakeAvailable(wxWidgets)

# --- Main application ---
add_executable(A2DPWB WIN32
    src/main.cpp
    src/wasapi_capture.cpp
    src/ldac_encoder.cpp
    src/aptxhd_encoder.cpp
    src/aptxll_encoder.cpp
    src/a2dp_sbc_encoder.cpp
    src/aac_encoder.cpp
    src/bt_device.cpp
    src/bt_adapter_enum.cpp
    src/btstack_transport.cpp
    src/btstack_link_key_db_file.c
    src/config_path.cpp
    src/a2dp_service.cpp
    src/wx_app.cpp
    src/wx_main_frame.cpp
    src/wx_profile_dialog.cpp
    src/wx_settings_dialog.cpp
    src/wx_about_dialog.cpp
    src/wx_firmware_dialog.cpp
    src/wx_zadig_dialog.cpp
    src/app_settings.cpp
    src/theme_manager.cpp
    src/audio_device_enum.cpp
    src/localization.cpp
    src/profile_manager.cpp
    src/system_integration.cpp
    src/zadig_helper.cpp
    resources/app.rc
)

set_target_properties(A2DPWB PROPERTIES OUTPUT_NAME "A2DPWB-${PROJECT_VERSION}")

target_include_directories(A2DPWB PRIVATE
    ${CMAKE_SOURCE_DIR}/extern/libldac/inc
    ${CMAKE_SOURCE_DIR}/extern/libopenaptx
    ${CMAKE_SOURCE_DIR}/extern/json/include
)

# libldac (LDAC codec)
if(TARGET ldac)
    target_link_libraries(A2DPWB PRIVATE ldac)
endif()

# libopenaptx (aptX / aptX HD codec)
if(TARGET openaptx)
    target_link_libraries(A2DPWB PRIVATE openaptx)
endif()

# BTstack (user-mode Bluetooth stack via WinUSB)
if(TARGET btstack)
    target_link_libraries(A2DPWB PRIVATE btstack)
endif()

# SBC encoder (BTstack's Bluedroid)
if(TARGET sbc_codec)
    target_link_libraries(A2DPWB PRIVATE sbc_codec)
endif()

# fdk-aac (AAC-LC encoder)
if(TARGET fdk-aac)
    target_link_libraries(A2DPWB PRIVATE fdk-aac)
    target_compile_definitions(A2DPWB PRIVATE AAC_ENCODER_AVAILABLE=1)
    target_include_directories(A2DPWB PRIVATE
        ${CMAKE_SOURCE_DIR}/extern/fdk-aac/libAACenc/include
        ${CMAKE_SOURCE_DIR}/extern/fdk-aac/libSYS/include
    )
endif()

# wxWidgets (core + base)
target_link_libraries(A2DPWB PRIVATE wx::core wx::base)

# Windows system libraries
target_link_libraries(A2DPWB PRIVATE
    ole32          # COM (CoInitializeEx, CoCreateInstance)
    uuid           # COM interface UUIDs (__uuidof)
    avrt           # Multimedia scheduling (AvSetMmThreadCharacteristics)
    bthprops       # Bluetooth device enumeration (BluetoothFindFirstDevice)
    ws2_32         # Winsock (Bluetooth address types)
    propsys        # Property store (PKEY_Device_FriendlyName)
    version        # GetFileVersionInfo (friendly app names)
    setupapi       # SetupDi (Zadig USB enumeration)
    shell32        # Shell_NotifyIcon (system tray)
    shcore         # SetProcessDpiAwareness (high-DPI)
    winmm          # timeBeginPeriod/timeEndPeriod (timer resolution)
)

# Application version
target_compile_definitions(A2DPWB PRIVATE
    APP_VERSION="${PROJECT_VERSION}"
)

# LDAC ABR (Adaptive Bit Rate) support - available when libldac includes abr/
if(EXISTS ${CMAKE_SOURCE_DIR}/extern/libldac/abr/src/ldacBT_abr.c)
    target_compile_definitions(A2DPWB PRIVATE LDAC_ABR_AVAILABLE=1)
    target_include_directories(A2DPWB PRIVATE
        ${CMAKE_SOURCE_DIR}/extern/libldac/abr/inc
    )
endif()

# Compile definitions
target_compile_definitions(A2DPWB PRIVATE
    _WIN32_WINNT=0x0A00   # Windows 10+
    NOMINMAX               # Avoid min/max macro conflicts
    WIN32_LEAN_AND_MEAN    # Reduce Windows header bloat
)

if(MSVC)
    target_compile_options(A2DPWB PRIVATE /W3 /utf-8)
    # Use main() entry point with WIN32 subsystem (no console window)
    target_link_options(A2DPWB PRIVATE /ENTRY:mainCRTStartup)
    # Manifest is embedded via app.rc; prevent linker from generating a duplicate
    target_link_options(A2DPWB PRIVATE /MANIFEST:NO)
else()
    target_compile_options(A2DPWB PRIVATE -Wall -Wextra)
endif()

# Embed language files into the executable at configure time
file(GLOB LANG_FILES "${CMAKE_CURRENT_SOURCE_DIR}/lang/*.json")
set(EMBEDDED_LANG_ENTRIES "")
foreach(LANG_FILE ${LANG_FILES})
    get_filename_component(LANG_CODE ${LANG_FILE} NAME_WE)
    file(READ ${LANG_FILE} LANG_CONTENT)
    string(APPEND EMBEDDED_LANG_ENTRIES "        {\"${LANG_CODE}\", R\"LANG_JSON(${LANG_CONTENT})LANG_JSON\"},\n")
endforeach()
configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/src/embedded_langs.h.in
    ${CMAKE_CURRENT_BINARY_DIR}/generated/embedded_langs.h
    @ONLY
)
target_include_directories(A2DPWB PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/generated)
