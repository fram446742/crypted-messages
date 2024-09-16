@echo off
setlocal

REM Build for x86_64-pc-windows-msvc target
cargo build --release --target x86_64-pc-windows-msvc

REM Check if the MSVC build was successful
if %errorlevel% == 0 (
    echo MSVC Build successful.
    if exist target\x86_64-pc-windows-msvc\release\crypted-messages.exe (
        ren target\x86_64-pc-windows-msvc\release\crypted-messages.exe target\x86_64-pc-windows-msvc\release\crypted-messages-x86_64-pc-windows-msvc.exe
    )
) else (
    echo MSVC Build failed.
    exit /b 1
)

@REM REM Build for x86_64-pc-windows-gnu target
@REM cargo build --release --target x86_64-pc-windows-gnu 

@REM REM Check if the GNU build was successful
@REM if %errorlevel% == 0 (
@REM     echo GNU Build successful.
@REM     if exist target\x86_64-pc-windows-gnu\release\crypted-messages.exe (
@REM         ren target\x86_64-pc-windows-gnu\release\crypted-messages.exe target\x86_64-pc-windows-gnu\release\crypted-messages-x86_64-pc-windows-gnu.exe
@REM     )
@REM ) else (
@REM     echo GNU Build failed.
@REM     exit /b 1
@REM )

REM Check if 'make' command exists
where make >nul 2>nul
if %errorlevel% == 0 (
    echo 'make' is already installed.
    make compile
    goto :EOF
)

REM Check for OS type and install 'make' accordingly
if "%OS%"=="Windows_NT" (
    echo Installing make using Chocolatey...
    choco install make
) else (
    echo Installing make using apt-get...
    sudo apt-get update
    sudo apt-get install make
)

REM Retry after installation
where make >nul 2>nul
if %errorlevel% == 0 (
    echo 'make' installed successfully.
    make compile
) else (
    echo Failed to install 'make'.
    exit /b 1
)


endlocal
