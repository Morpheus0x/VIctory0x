@echo off
rem Builds victory.exe for this baseline. No path is baked in: pass --repo etc.
rem Same command line as run.bat; options this script does not use are ignored.
setlocal EnableExtensions
set "REPO="
set "GAME="
set "DATA="
set "TARGET="
set "EXE="
set "LOG="
set "SKIP="
set "TEXTURE_BACK="
set "REST="
set "PROFILE=diag"
set "FEATURES=diagnostics"
set "AGILITY=1"
set "WARM=1"

:parse
if "%~1"=="" goto parsed
set "OPT=%~1"
set "VAL=%~2"
if /i "%OPT%"=="--help" goto usage
if /i "%OPT%"=="-h" goto usage
if /i "%OPT%"=="--" goto rest
if "%VAL%"=="" goto missing
if /i "%OPT%"=="--repo" ( set "REPO=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--game" ( set "GAME=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--data" ( set "DATA=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--profile" ( set "PROFILE=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--target" ( set "TARGET=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--features" ( set "FEATURES=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--agility" ( set "AGILITY=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--exe" ( set "EXE=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--log" ( set "LOG=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--warm" ( set "WARM=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--skip-pipelines" ( set "SKIP=%VAL%" & set "KNOWN=1" )
if /i "%OPT%"=="--texture-back" ( set "TEXTURE_BACK=%VAL%" & set "KNOWN=1" )
if not defined KNOWN (
  echo [!] unknown option: %OPT%
  goto usage
)
set "KNOWN="
shift
shift
goto parse

:rest
shift
set "REST=%*"
goto parsed

:missing
echo [!] %OPT% needs a value
goto usage

:usage
echo usage: %~nx0 --repo DIR [--game FILE] [--data DIR] [--profile NAME]
echo               [--target TRIPLE] [--features LIST] [--agility 0^|1]
echo               [--exe FILE] [--log FILE] [--warm 0^|1]
echo               [--skip-pipelines LIST] [--texture-back LIST] [-- title args...]
echo.
echo   --repo DIR        repository root (required)
echo   --game FILE       title executable (run needs it; build ignores it)
echo   --data DIR        data directory (run needs it; build ignores it)
echo   --profile NAME    cargo profile (default diag)
echo   --target TRIPLE   cargo target triple (default: native)
echo   --features LIST   cargo features (default diagnostics)
echo   --agility 0^|1    stage the D3D12 Agility SDK first (default 1)
echo   --exe FILE        victory.exe (default under the repository's target)
echo   --log FILE        run log (run only; default under --data)
echo   --warm 0^|1       shader warm-up before the run (default 1)
echo   --skip-pipelines LIST  VICTORY_SKIP_PIPELINES for the run (default empty)
echo   --texture-back LIST    VICTORY_TEXTURE_BACK for the run (baseline default)
echo   --                what follows goes to the title
exit /b 2

:parsed
if not defined REPO (
  echo [!] --repo is required
  goto usage
)
if not exist "%REPO%\Cargo.toml" (
  echo [!] no Cargo.toml in %REPO%
  exit /b 2
)

if "%AGILITY%"=="1" (
  echo [1/2] staging the D3D12 Agility SDK (shader model 6.6 on Windows 10)
  cargo run --quiet --manifest-path "%REPO%\Cargo.toml" -p xtask -- agility
  if errorlevel 1 ( echo [!] xtask agility failed & exit /b 2 )
)
echo [2/2] building victory.exe (profile %PROFILE%)
if defined TARGET (
  cargo build --locked --manifest-path "%REPO%\Cargo.toml" --profile "%PROFILE%" --target "%TARGET%" -p victory --features "%FEATURES%"
) else (
  cargo build --locked --manifest-path "%REPO%\Cargo.toml" --profile "%PROFILE%" -p victory --features "%FEATURES%"
)
if errorlevel 1 ( echo [!] build failed & exit /b 2 )
if not defined EXE (
  if defined TARGET (
    set "EXE=%REPO%\target\%TARGET%\%PROFILE%\victory.exe"
  ) else (
    set "EXE=%REPO%\target\%PROFILE%\victory.exe"
  )
)
echo.
echo built: %EXE%
endlocal & exit /b 0
