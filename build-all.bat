@echo off
setlocal

REM Check if 'cargo' command exists
where cargo >nul 2>nul
if %errorlevel% == 0 (
    echo 'cargo' is already installed.
) else (
    echo You need to install Rust and Cargo first.
    echo Visit https://www.rust-lang.org/tools/install for more information.
    pause
    exit /b 1
)

REM Ask the user if they want to compile only for Windows targets or for all targets
set /p targets=Do you want to compile only for Windows targets (w) or for all targets (a)? [w/a]:
if /i not "%targets%"=="w" if /i not "%targets%"=="a" (
    echo Invalid option selected. Exiting...
    pause
    exit /b 1
)

REM Build for the current target
cargo build --release
if %errorlevel% == 0 (
    echo Current Build successful.
) else (
    echo Current Build failed.
    pause
    exit /b 1
)

REM Check if 'choco' command exists
where choco >nul 2>nul
if %errorlevel% == 0 (
    echo 'choco' is already installed.
) else (
    echo You need to install Chocolatey first.
    echo Visit https://chocolatey.org/install for more information.
    pause
    exit /b 1
)

REM Check if 'gcc' command exists
where gcc >nul 2>nul
if %errorlevel% == 0 (
    echo 'gcc' is already installed.
) else (
    echo Installing gcc using Chocolatey...
    choco install mingw -y
    where gcc >nul 2>nul
    if %errorlevel% == 0 (
        echo 'gcc' installed successfully.
    ) else (
        echo Failed to install 'gcc'.
        pause
        exit /b 1
    )
)

REM Compile based on user choice
if /i "%targets%"=="w" (
    echo Compiling only for Windows targets...

    REM Build for x86_64-pc-windows-msvc target
    cargo build --release --target x86_64-pc-windows-msvc
    if %errorlevel% == 0 (
        echo MSVC Build successful.
        if exist target\x86_64-pc-windows-msvc\release\crypted-messages.exe (
            ren target\x86_64-pc-windows-msvc\release\crypted-messages.exe crypted-messages-x86_64-pc-windows-msvc.exe
        )
    ) else (
        echo MSVC Build failed.
    )

    REM Build for x86_64-pc-windows-gnu target
    cargo build --release --target x86_64-pc-windows-gnu
    if %errorlevel% == 0 (
        echo GNU Build successful.
        if exist target\x86_64-pc-windows-gnu\release\crypted-messages.exe (
            ren target\x86_64-pc-windows-gnu\release\crypted-messages.exe crypted-messages-x86_64-pc-windows-gnu.exe
        )
    ) else (
        echo GNU Build failed.
    )

) else if /i "%targets%"=="a" (
    echo Compiling for all targets...

    REM Build for x86_64-pc-windows-msvc target
    cargo build --release --target x86_64-pc-windows-msvc
    if %errorlevel% == 0 (
        echo MSVC Build successful.
        if exist target\x86_64-pc-windows-msvc\release\crypted-messages.exe (
            ren target\x86_64-pc-windows-msvc\release\crypted-messages.exe crypted-messages-x86_64-pc-windows-msvc.exe
        )
    ) else (
        echo MSVC Build failed.
    )

    REM Build for x86_64-pc-windows-gnu target
    cargo build --release --target x86_64-pc-windows-gnu
    if %errorlevel% == 0 (
        echo GNU Build successful.
        if exist target\x86_64-pc-windows-gnu\release\crypted-messages.exe (
            ren target\x86_64-pc-windows-gnu\release\crypted-messages.exe crypted-messages-x86_64-pc-windows-gnu.exe
        )
    ) else (
        echo GNU Build failed.
    )

    REM Check if 'make' command exists
    where make >nul 2>nul
    if %errorlevel% == 0 (
        echo 'make' is already installed.
        make compile
    ) else (
        echo Installing make using Chocolatey...
        choco install make -y
        where make >nul 2>nul
        if %errorlevel% == 0 (
            echo 'make' installed successfully.
            make compile
        ) else (
            echo Failed to install 'make'.
            exit /b 1
        )
    )

)

endlocal
pause
