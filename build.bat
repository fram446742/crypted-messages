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

REM chech if 'cross' command exists
where cross >nul 2>nul
if %errorlevel% == 0 (
    echo 'cross' is already installed.
) else (
    echo Installing cross using Cargo...
    cargo install cross
    where cross >nul 2>nul
    if %errorlevel% == 0 (
        echo 'cross' installed successfully.
    ) else (
        echo Failed to install 'cross'.
        pause
        exit /b 1
    )
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

REM Once the build is complete, ask the user if they want to copy the executables to the 'bin' directory
set /p copy=Do you want to copy the executables to the 'bin' directory? [y/n]:
if /i "%copy%"=="y" (
    if exist bin\ (
        del /q bin\*
    ) else (
        mkdir bin
    )

    REM Search for all the executables like crypted-messages*.exe and copy them to the 'bin' directory and do it for all targets inside the folders like (architecture) and (release) except for the ones that have a .d extension

    for /r target %%f in (crypted-messages*) do (
        if "%%~xf"=="" (
            copy "%%f" bin
        ) else if "%%~xf"==".exe" (
            copy "%%f" bin
        )
    )

    echo Executables copied to the 'bin' directory.
) else (
    echo Executables not copied to the 'bin' directory.
)


endlocal
pause
