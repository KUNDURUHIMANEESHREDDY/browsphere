@echo off
setlocal enabledelayedexpansion
title Building Browsphere Standalone Installer
echo ========================================================
echo Building Standalone Browsphere Windows Installer
echo ========================================================
echo.

set "ROOT_DIR=%~dp0"

where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Rust cargo not found in PATH. Install from https://rustup.rs/
    pause
    exit /b 1
)

where npm >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Node.js npm not found in PATH. Install from https://nodejs.org/
    pause
    exit /b 1
)

if exist "%ROOT_DIR%ui\package.json" (
    echo [1/4] Installing UI dependencies and compiling Tailwind CSS...
    pushd "%ROOT_DIR%ui"
    call npm ci --no-audit --no-fund
    if errorlevel 1 (
        popd
        echo [ERROR] npm ci failed.
        pause
        exit /b 1
    )
    call npm run build:css
    if errorlevel 1 (
        popd
        echo [ERROR] Tailwind CSS build failed.
        pause
        exit /b 1
    )
    popd
)

echo [2/4] Checking Tauri CLI...
cargo tauri --version >nul 2>nul
if errorlevel 1 (
    echo Installing tauri-cli - one-time setup...
    cargo install tauri-cli --version "^2" --locked
    if errorlevel 1 (
        echo [ERROR] Failed to install tauri-cli.
        pause
        exit /b 1
    )
)

echo [3/4] Compiling standalone Windows app and NSIS/MSI installers...
pushd "%ROOT_DIR%src-tauri"
cargo tauri build --bundles nsis
if errorlevel 1 (
    popd
    echo [ERROR] Build failed. Check the Rust compiler output above.
    pause
    exit /b 1
)
popd

echo [4/4] Copying standalone setup.exe to dist folder...
if not exist "%ROOT_DIR%dist" mkdir "%ROOT_DIR%dist"
set "FOUND_SETUP=0"
for %%D in ("%ROOT_DIR%target\release\bundle" "%ROOT_DIR%src-tauri\target\release\bundle") do (
    if exist "%%~D\nsis" (
        for %%F in ("%%~D\nsis\*-setup.exe") do (
            if exist "%%~fF" (
                copy /Y "%%~fF" "%ROOT_DIR%dist\Browsphere-setup.exe" >nul
                if errorlevel 1 (
                    echo [ERROR] Failed to copy installer to dist.
                    exit /b 1
                )
                set "FOUND_SETUP=1"
            )
        )
    )
    if exist "%%~D\msi" (
        for %%F in ("%%~D\msi\*.msi") do (
            if exist "%%~fF" copy /Y "%%~fF" "%ROOT_DIR%dist\" >nul
        )
    )
)

if "!FOUND_SETUP!"=="1" (
    if defined PFX_PATH (
        if exist "!PFX_PATH!" (
            where signtool >nul 2>nul
            if not errorlevel 1 (
                echo Signing dist\Browsphere-setup.exe with Authenticode...
                if defined PFX_PASSWORD (
                    signtool sign /f "!PFX_PATH!" /p "!PFX_PASSWORD!" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "%ROOT_DIR%dist\Browsphere-setup.exe"
                ) else (
                    signtool sign /f "!PFX_PATH!" /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 "%ROOT_DIR%dist\Browsphere-setup.exe"
                )
                if errorlevel 1 (
                    echo [ERROR] Installer signing failed.
                    exit /b 1
                )
            )
        )
    )
)

echo.
if "!FOUND_SETUP!"=="1" (
    echo ========================================================
    echo DONE! Your standalone installer is ready at:
    echo   %ROOT_DIR%dist\Browsphere-setup.exe
    echo ========================================================
) else (
    echo [ERROR] Build finished, but setup.exe was not found in expected bundle path.
    echo Check target\release\bundle\ or src-tauri\target\release\bundle\
    exit /b 1
)
pause
endlocal
