@echo off
rem Builds victory.exe for this baseline from the repository this script sits in
rem (or from %1 when given). Always waits at the end and reports the result.
setlocal EnableExtensions
set "REPO=%~1"
if not defined REPO set "REPO=%CD%"
if not exist "%REPO%\Cargo.toml" (
  echo [!] no Cargo.toml in %REPO% - run this from the repository, or pass it as the first argument
  goto fail
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo [!] cargo is not on PATH - install Rust from https://rustup.rs
  goto fail
)

echo [1/2] staging the D3D12 Agility SDK (shader model 6.6 on Windows 10)
cargo run --quiet --manifest-path "%REPO%\Cargo.toml" -p xtask -- agility
if errorlevel 1 (
  echo [!] xtask agility failed
  goto fail
)

echo [2/2] building victory.exe (profile diag)
cargo build --locked --manifest-path "%REPO%\Cargo.toml" --profile diag -p victory --features diagnostics
if errorlevel 1 (
  echo [!] the build failed
  goto fail
)

set "EXE=%REPO%\target\diag\victory.exe"
if not exist "%EXE%" (
  echo [!] cargo reported success but %EXE% is missing
  goto fail
)
echo.
echo [OK] BUILD SUCCEEDED
echo      %EXE%
echo.
pause
endlocal & exit /b 0

:fail
echo.
echo [X] BUILD FAILED
echo.
pause
endlocal & exit /b 2
