# Read by CMake for SDL's build on Windows (see .cargo/config.toml). SDL picks its C runtime from
# this variable rather than from the /MT the build passes, and every part of a program has to link
# the same one: the static one, so it starts without the Visual C++ Redistributable.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")
