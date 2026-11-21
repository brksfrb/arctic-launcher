if(DEFINED ENV{CMAKE_TOOLCHAIN_FILE_x86_64_pc_windows_msvc})
    include("$ENV{CMAKE_TOOLCHAIN_FILE_x86_64_pc_windows_msvc}")
endif()

# Opus chooses its own CRT even when Cargo and cargo-xwin request a static CRT.
set(OPUS_STATIC_RUNTIME ON CACHE BOOL "Link Opus with the static MSVC CRT" FORCE)
