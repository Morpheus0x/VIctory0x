@echo off
rem Runs victory.exe for this baseline. No path is baked in: pass --repo etc.
rem Same command line as build.bat; options this script does not use are ignored.
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
if not defined GAME (
  echo [!] --game is required
  goto usage
)
if not defined DATA (
  echo [!] --data is required
  goto usage
)
for %%I in ("%GAME%") do set "GAMEDIR=%%~dpI"
set "CACHE=%DATA%\cache"
set "DATADIR=%DATA%\data"
set "LOGS=%DATA%\logs"
if not exist "%GAMEDIR%" (
  echo [!] game directory not found: %GAMEDIR%
  exit /b 2
)
for %%D in ("%DATA%" "%CACHE%" "%DATADIR%" "%LOGS%") do if not exist "%%~D" mkdir "%%~D"
if not defined EXE (
  if defined TARGET (
    set "EXE=%REPO%\target\%TARGET%\%PROFILE%\victory.exe"
  ) else (
    set "EXE=%REPO%\target\%PROFILE%\victory.exe"
  )
)
if not exist "%EXE%" (
  echo [!] %EXE% not found - run build.bat with the same arguments first
  exit /b 2
)
if not defined LOG set "LOG=%LOGS%\run.log"

set "VICTORY_CACHE_DIR=%CACHE%"
set "VICTORY_DATA_DIR=%DATADIR%"
set VICTORY_LOG=warn
set RUST_BACKTRACE=1
set VICTORY_FRAME_LOG=1
set VICTORY_FRAME_LOG_FILE=%LOGS%\frame.log
if defined SKIP set "VICTORY_SKIP_PIPELINES=%SKIP%"
if defined TEXTURE_BACK (
  set "VICTORY_TEXTURE_BACK=%TEXTURE_BACK%"
) else (
  set "VICTORY_TEXTURE_BACK=64x64f16,128x128f36"
)
set "CONFIG="
if exist "%DATA%\config.toml" set "CONFIG=%DATA%\config.toml"
if not defined CONFIG if exist "%REPO%\victory.toml" set "CONFIG=%REPO%\victory.toml"
if defined CONFIG set "VICTORY_CONFIG=%CONFIG%"

set "STAMP=%CACHE%\warm.sha"
for /f %%H in ('powershell -NoProfile -Command "(Get-FileHash -Algorithm SHA256 -LiteralPath '%EXE%').Hash.ToLower()"') do set "SHA=%%H"
set "OLD="
if exist "%STAMP%" for /f "usebackq delims=" %%S in ("%STAMP%") do set "OLD=%%S"
if "%WARM%"=="0" goto run
if /i "%OLD%"=="%SHA%" goto run

:warm
echo [warm] translating every shader for this build into %CACHE%
if exist "%GAMEDIR%shaders.rpf" (
  "%EXE%" translate --warm --jobs %NUMBER_OF_PROCESSORS% "%GAMEDIR%shaders.rpf"
) else (
  "%EXE%" translate --warm --jobs %NUMBER_OF_PROCESSORS% "%GAMEDIR%"
)
if errorlevel 1 (
  echo [!] shader warm-up failed - running anyway
) else (
  >"%STAMP%" echo %SHA%
)

:run
echo === run %DATE% %TIME% === >> "%LOG%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$enc = New-Object -TypeName System.Text.UTF8Encoding -ArgumentList $false; $sw = New-Object -TypeName System.IO.StreamWriter -ArgumentList '%LOG%', $true, $enc; $sw.AutoFlush = $true; & '%EXE%' run '%GAME%' --execute --console series-x --unimplemented zero %REST% 2>&1 | ForEach-Object { $_; $sw.WriteLine($_) }; $code = $LASTEXITCODE; $sw.Close(); exit $code"

echo.
echo exit %ERRORLEVEL%
echo run log: %LOG%
endlocal & exit /b %ERRORLEVEL%
