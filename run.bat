@echo off
rem Runs victory.exe for this baseline. <game-dir> and <data-dir> are required;
rem the repository is the current directory unless given as the third argument.
setlocal EnableExtensions
if "%~2"=="" goto usage
set "GAMEDIR=%~1"
for %%I in ("%GAMEDIR%\.") do set "GAMEDIR=%%~fI"
set "GAME=%GAMEDIR%\game_scarlett_master.exe"
set "DATA=%~2"
for %%I in ("%DATA%\.") do set "DATA=%%~fI"
set "REPO=%~3"
if not defined REPO set "REPO=%CD%"
set "CACHE=%DATA%\cache"
set "DATADIR=%DATA%\data"
set "LOGS=%DATA%\logs"
set "EXE=%REPO%\target\diag\victory.exe"

if not exist "%GAME%" (
  echo [!] game_scarlett_master.exe not found in %GAMEDIR%
  exit /b 2
)
if not exist "%EXE%" (
  echo [!] %EXE% not found - run build.bat first
  exit /b 2
)
for %%D in ("%DATA%" "%CACHE%" "%DATADIR%" "%LOGS%") do if not exist "%%~D" mkdir "%%~D"
set "BRANCH=coolfox"
for /f %%T in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd_HHmmss"') do set "TS=%%T"
set "LOG=%LOGS%\run_%TS%_%BRANCH%.log"

goto body

:usage
echo usage: %~nx0 ^<game-dir^> ^<data-dir^> [repo]
echo.
echo   game-dir   folder holding game_scarlett_master.exe
echo   data-dir   root for cache, data (saves) and logs
echo   repo       repository root (default: the current directory)
exit /b 2

:body

set "VICTORY_GAME_DIR=%GAMEDIR%"
set "VICTORY_CACHE_DIR=%CACHE%"
set "VICTORY_DATA_DIR=%DATADIR%"
rem Quiet by default; add victory_gpu::host=info for the 5-second frame breakdown.
if not defined VICTORY_LOG set VICTORY_LOG=warn
set RUST_BACKTRACE=1
rem The fork's own launcher (scripts/play.ps1) enables these; without them
rem THIN lists, joined submissions, relay/land and card steps stay off.
set VICTORY_TEXTURE_NO_WAIT=1
set VICTORY_RELAY=2
set VICTORY_WIDE_BACK=1
set VICTORY_CARD_REHASH_RUNS=1
set VICTORY_JOIN_COPIES=1
set VICTORY_INDIRECT_WINDOW=1
set VICTORY_RELAY_COPY=1
set VICTORY_CARD_STEPS=2
set VICTORY_INDIRECT_BATCH=2
set VICTORY_MAP_THREADS=1
set VICTORY_TITLE_PRIORITIES=0
set VICTORY_CARD_AWAKE=1
set VICTORY_SHOW_AFTER=1
set VICTORY_D3D12_DRED=light
set VICTORY_THIN=1
set VICTORY_JOIN_PARTS=8
set VICTORY_UPLOAD_AHEAD=1
rem The fork's own render settings (scripts/play.ps1).
set VICTORY_AMBIENT=0.8
set VICTORY_WINDOW_MAX=1920x1080
rem Record the pipelines the title creates, so `victory warm --used` can make
rem them ahead of later runs (one entry per new pipeline; tiny).
set VICTORY_PIPELINE_BOOK=1
rem This fork translates the pipelines the old skip list worked around.
set VICTORY_SKIP_PIPELINES=none

set "STAMP=%CACHE%\warm.sha"
for /f %%H in ('powershell -NoProfile -Command "(Get-FileHash -Algorithm SHA256 -LiteralPath '%EXE%').Hash.ToLower()"') do set "SHA=%%H"
set "OLD="
if exist "%STAMP%" for /f "usebackq delims=" %%S in ("%STAMP%") do set "OLD=%%S"
if /i "%OLD%"=="%SHA%" goto pipelines

:warm
echo [warm] translating every shader for this build into %CACHE%
if exist "%GAMEDIR%shaders.rpf" (
  "%EXE%" shaders "%GAMEDIR%shaders.rpf"
) else (
  "%EXE%" shaders "%GAMEDIR%"
)
if errorlevel 1 (
  echo [!] shader warm-up failed - running anyway
) else (
  >"%STAMP%" echo %SHA%
)

:pipelines
if not exist "%CACHE%\pipelines\known-pipelines.bin" goto run
echo [warm] making host pipelines earlier runs recorded (victory warm --used)
"%EXE%" warm --used
if errorlevel 1 echo [!] pipeline warm-up failed - running anyway

:run
echo === run %DATE% %TIME% === >> "%LOG%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$enc = New-Object -TypeName System.Text.UTF8Encoding -ArgumentList $false; $sw = New-Object -TypeName System.IO.StreamWriter -ArgumentList '%LOG%', $true, $enc; $sw.AutoFlush = $true; & '%EXE%' run '%GAME%' --execute --console series-x --unimplemented zero 2>&1 | ForEach-Object { $_; $sw.WriteLine($_) }; $code = $LASTEXITCODE; $sw.Close(); exit $code"

echo.
echo exit %ERRORLEVEL%
echo run log: %LOG%
endlocal & exit /b %ERRORLEVEL%
