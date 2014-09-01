@echo off
SET PATH=C:\WINDOWS;C:\WINDOWS\System32;C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\bin\amd64;A:\rust64\bin;A:\LLVM\bin
set INCLUDE=C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\include;C:\Program Files (x86)\Windows Kits\8.1\Include\um;C:\Program Files (x86)\Windows Kits\8.1\Include\shared
set LIB=C:\Program Files (x86)\Windows Kits\8.1\Lib\winv6.3\um\x64;C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\lib\amd64
rustc ..\src\libcore\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
rustc ..\src\liblibc\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
rustc ..\src\libunicode\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
rustc ..\src\liballoc\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
rustc ..\src\libcollections\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
rustc ..\src\librand\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
cl rustrt_native.c /c /nologo
lib rustrt_native.obj /nologo
del rustrt_native.obj
rustc ..\src\librustrt\lib.rs --target=x86_64-pc-windows -Car=llvm-ar -L. --crate-type=rlib
rustc test.rs --emit=obj --target=x86_64-pc-windows -L.
link "test.o" "libcore.rlib" "user32.lib" "MSVCRT.lib" /SUBSYSTEM:CONSOLE /nologo
del test.o
pause
