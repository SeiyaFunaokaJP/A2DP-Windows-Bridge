# make_dist.cmake - assemble the release package (run via `cmake -P`).
#
# Invoked by the A2DPWB_dist target (app/CMakeLists.txt). Required -D variables:
#   APP_VERSION   project version (e.g. 1.0.5)
#   APP_EXE       path of the built A2DPWB.exe
#   BUILD_CONFIG  build configuration of APP_EXE (must be Release)
#   SRC_DIR       repository root
#   WX_SRC_DIR    wxWidgets FetchContent source directory
#   WX_TAG        wxWidgets git tag used by FetchContent
#   DIST_ROOT     output directory (build/dist)
#   GIT_EXE       git executable (may be empty / NOTFOUND)
#
# Produces:
#   ${DIST_ROOT}/A2DPWB-${APP_VERSION}/            (unpacked package)
#   ${DIST_ROOT}/A2DPWB-${APP_VERSION}-win64.zip

cmake_minimum_required(VERSION 3.16)

foreach(var APP_VERSION APP_EXE BUILD_CONFIG SRC_DIR WX_SRC_DIR WX_TAG DIST_ROOT)
    if(NOT DEFINED ${var} OR "${${var}}" STREQUAL "")
        message(FATAL_ERROR "make_dist.cmake: ${var} is not set")
    endif()
endforeach()

if(NOT BUILD_CONFIG STREQUAL "Release")
    message(FATAL_ERROR "A2DPWB_dist: release packages must be built from the "
        "Release configuration (got '${BUILD_CONFIG}'). "
        "Use: cmake --build build --target A2DPWB_dist --config Release")
endif()

set(PKG_NAME "A2DPWB-${APP_VERSION}")
set(PKG_DIR "${DIST_ROOT}/${PKG_NAME}")
set(ZIP_FILE "${DIST_ROOT}/${PKG_NAME}-win64.zip")

file(REMOVE_RECURSE "${PKG_DIR}")
file(REMOVE "${ZIP_FILE}")
file(MAKE_DIRECTORY "${PKG_DIR}/licenses")

# copy_required(<src> <dest relative to PKG_DIR>)
function(copy_required src dest)
    if(NOT EXISTS "${src}")
        message(FATAL_ERROR "A2DPWB_dist: required file not found: ${src}\n"
            "Run: git submodule update --init --recursive (and re-run CMake "
            "configure so wxWidgets is fetched).")
    endif()
    configure_file("${src}" "${PKG_DIR}/${dest}" COPYONLY)
endfunction()

# --- Executable and top-level documents ---
copy_required("${APP_EXE}"                         "A2DPWB.exe")
copy_required("${SRC_DIR}/LICENSE"                 "LICENSE")
copy_required("${SRC_DIR}/THIRD_PARTY_LICENSES.md" "THIRD_PARTY_LICENSES.md")

# --- Third-party license texts, taken from the pinned sources ---
set(EXT "${SRC_DIR}/extern")
copy_required("${EXT}/btstack/LICENSE"       "licenses/BTstack-LICENSE.txt")
copy_required("${EXT}/libldac/LICENSE"       "licenses/libldac-LICENSE.txt")
copy_required("${EXT}/libldac/NOTICE"        "licenses/libldac-NOTICE.txt")
# Bluedroid SBC codec (bundled in BTstack 3rd-party/bluedroid) is Apache-2.0
# but ships no license file of its own; libldac's LICENSE is the full text.
copy_required("${EXT}/libldac/LICENSE"       "licenses/Apache-2.0.txt")
copy_required("${EXT}/fdk-aac/NOTICE"        "licenses/fdk-aac-NOTICE.txt")
copy_required("${EXT}/libopenaptx/COPYING"   "licenses/libopenaptx-LGPL-2.1.txt")
copy_required("${EXT}/json/LICENSE.MIT"      "licenses/nlohmann-json-LICENSE.MIT.txt")
copy_required("${WX_SRC_DIR}/docs/licence.txt"            "licenses/wxWidgets-licence.txt")
copy_required("${WX_SRC_DIR}/docs/lgpl.txt"               "licenses/wxWidgets-lgpl.txt")
copy_required("${WX_SRC_DIR}/src/zlib/LICENSE"            "licenses/zlib.txt")
copy_required("${WX_SRC_DIR}/src/png/LICENSE"             "licenses/libpng-LICENSE.txt")
copy_required("${WX_SRC_DIR}/3rdparty/nanosvg/LICENSE.txt" "licenses/nanosvg-LICENSE.txt")

# Sanity check: libopenaptx must be the LGPL-2.1 release (0.2.0), not GPL-3.0.
file(READ "${EXT}/libopenaptx/COPYING" _aptx_license LIMIT 400)
if(NOT _aptx_license MATCHES "GNU LESSER GENERAL PUBLIC LICENSE[ \t\r\n]+Version 2\\.1")
    message(FATAL_ERROR "A2DPWB_dist: extern/libopenaptx/COPYING is not LGPL-2.1. "
        "libopenaptx must be pinned to 0.2.0 (see THIRD_PARTY_LICENSES.md).")
endif()

# --- SOURCE.txt (written offer / source location) ---
set(REPO_URL "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge")
set(HEAD_COMMIT "unknown (git not available)")
set(SUBMODULES "  (git not available)")
set(DIRTY_NOTE "")
if(GIT_EXE AND EXISTS "${GIT_EXE}")
    execute_process(COMMAND "${GIT_EXE}" rev-parse HEAD
        WORKING_DIRECTORY "${SRC_DIR}"
        OUTPUT_VARIABLE HEAD_COMMIT OUTPUT_STRIP_TRAILING_WHITESPACE
        RESULT_VARIABLE _rc)
    if(NOT _rc EQUAL 0)
        message(FATAL_ERROR "A2DPWB_dist: git rev-parse HEAD failed")
    endif()
    execute_process(COMMAND "${GIT_EXE}" submodule status
        WORKING_DIRECTORY "${SRC_DIR}"
        OUTPUT_VARIABLE SUBMODULES OUTPUT_STRIP_TRAILING_WHITESPACE
        RESULT_VARIABLE _rc)
    if(NOT _rc EQUAL 0)
        message(FATAL_ERROR "A2DPWB_dist: git submodule status failed")
    endif()
    execute_process(COMMAND "${GIT_EXE}" status --porcelain --untracked-files=no
        WORKING_DIRECTORY "${SRC_DIR}"
        OUTPUT_VARIABLE _status OUTPUT_STRIP_TRAILING_WHITESPACE)
    if(NOT _status STREQUAL "")
        set(DIRTY_NOTE "\nNOTE: this package was built from a working tree with uncommitted\nlocal modifications on top of the commit above.\n")
    endif()
else()
    message(WARNING "A2DPWB_dist: git not found; SOURCE.txt will not list commits")
endif()

file(WRITE "${PKG_DIR}/SOURCE.txt"
"A2DP Windows Bridge (A2DPWB) ${APP_VERSION}

Source code
-----------
Repository: ${REPO_URL}
Commit:     ${HEAD_COMMIT}
${DIRTY_NOTE}
Pinned submodules (git submodule status: commit, path, description):
${SUBMODULES}

wxWidgets: https://github.com/wxWidgets/wxWidgets (tag ${WX_TAG}),
           fetched at build time by CMake FetchContent (app/CMakeLists.txt).

The complete corresponding source code of this program and of every
library statically linked into A2DPWB.exe - including the LGPL-2.1-or-later
libopenaptx and the Fraunhofer FDK AAC codec - is available free of charge
at the repository URL and commits listed above. It contains everything
needed to rebuild and relink the executable (e.g. with a modified version
of libopenaptx):

    git clone --recursive ${REPO_URL}.git
    git checkout ${HEAD_COMMIT}
    git submodule update --init --recursive

See README.md in the repository for build instructions, and LICENSE,
THIRD_PARTY_LICENSES.md and the licenses/ folder in this package for the
license terms of each component.
")

# --- Zip (paths inside are relative: A2DPWB-<ver>/...) ---
execute_process(
    COMMAND "${CMAKE_COMMAND}" -E tar cf "${ZIP_FILE}" --format=zip "${PKG_NAME}"
    WORKING_DIRECTORY "${DIST_ROOT}"
    RESULT_VARIABLE _rc)
if(NOT _rc EQUAL 0)
    message(FATAL_ERROR "A2DPWB_dist: failed to create ${ZIP_FILE}")
endif()

message(STATUS "Release package: ${ZIP_FILE}")
