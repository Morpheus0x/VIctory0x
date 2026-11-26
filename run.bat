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
set "LOG=%LOGS%\run.log"
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

goto body

:usage
echo usage: %~nx0 ^<game-dir^> ^<data-dir^> [repo]
echo.
echo   game-dir   folder holding game_scarlett_master.exe
echo   data-dir   root for cache, data (saves) and logs
echo   repo       repository root (default: the current directory)
exit /b 2

:body

set "VICTORY_CACHE_DIR=%CACHE%"
set "VICTORY_DATA_DIR=%DATADIR%"
set VICTORY_LOG=warn
set RUST_BACKTRACE=1
set VICTORY_FRAME_LOG=1
set VICTORY_FRAME_LOG_FILE=%LOGS%\frame.log
set VICTORY_AMBIENT=0.6
set VICTORY_HAIR=1
set VICTORY_TEXTURE_BACK=64x64f16,128x128f36
set VICTORY_SKIP_PIPELINES=4e35b816f2f91d68,b3ba0ef9ee352720,5d627391703fcb4c,ec6e4c363cf9ef2b,c89489afd89ee7f0,45f081396c2c59ba,63083c777948927a,0bd4d80ad23b106b

set "CONFIG="
if exist "%DATA%\config.toml" set "CONFIG=%DATA%\config.toml"
if not defined CONFIG if exist "%REPO%\victory.toml" set "CONFIG=%REPO%\victory.toml"
if defined CONFIG set "VICTORY_CONFIG=%CONFIG%"

set "STAMP=%CACHE%\warm.sha"
for /f %%H in ('powershell -NoProfile -Command "(Get-FileHash -Algorithm SHA256 -LiteralPath '%EXE%').Hash.ToLower()"') do set "SHA=%%H"
set "OLD="
if exist "%STAMP%" for /f "usebackq delims=" %%S in ("%STAMP%") do set "OLD=%%S"
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
echo === run %DATE% %TIME% (%CONFIG%) === >> "%LOG%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$enc = New-Object -TypeName System.Text.UTF8Encoding -ArgumentList $false; $sw = New-Object -TypeName System.IO.StreamWriter -ArgumentList '%LOG%', $true, $enc; $sw.AutoFlush = $true; & '%EXE%' run '%GAME%' --execute --console series-x --unimplemented zero 2>&1 | ForEach-Object { $_; $sw.WriteLine($_) }; $code = $LASTEXITCODE; $sw.Close(); exit $code"

echo.
echo exit %ERRORLEVEL%
echo run log  : %LOG%
echo frame log: %VICTORY_FRAME_LOG_FILE%
endlocal & exit /b %ERRORLEVEL%
