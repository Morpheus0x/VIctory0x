@echo off
rem Builds victory.exe for this baseline from the repository this script sits in
rem (or from %1 when given). No other options.
setlocal EnableExtensions
set "REPO=%~1"
if not defined REPO set "REPO=%CD%"
if not exist "%REPO%\Cargo.toml" (
  echo [!] no Cargo.toml in %REPO% - run this from the repository, or pass it as the first argument
  exit /b 2
)

echo [1/2] staging the D3D12 Agility SDK (shader model 6.6 on Windows 10)
cargo run --quiet --manifest-path "%REPO%\Cargo.toml" -p xtask -- agility
if errorlevel 1 (
  echo [!] xtask agility failed
  exit /b 2
)

echo [2/2] building victory.exe (profile diag)
cargo build --locked --manifest-path "%REPO%\Cargo.toml" --profile diag -p victory --features diagnostics
if errorlevel 1 (
  echo [!] build failed
  exit /b 2
)

echo.
echo built: %REPO%\target\diag\victory.exe
endlocal & exit /b 0
