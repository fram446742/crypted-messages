@echo off
setlocal

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
