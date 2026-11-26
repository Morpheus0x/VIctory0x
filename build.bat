@echo off
rem Builds victory.exe for this baseline from the repository this script sits in
rem (or from %1 when given). Pulls the checked-out branch from its remote first
rem (rebase, never a merge commit), then always waits at the end and reports
rem the result.
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

echo [1/3] pulling the current branch from its remote
where git >nul 2>nul
if errorlevel 1 (
  echo [!] git is not on PATH - building the checkout as it is
  goto update-done
)
set "BRANCH="
for /f "delims=" %%b in ('git -C "%REPO%" rev-parse --abbrev-ref HEAD 2^>nul') do set "BRANCH=%%b"
if not defined BRANCH (
  echo [!] %REPO% is not a git checkout - building it as it is
  goto update-done
)
if /i "%BRANCH%"=="HEAD" (
  echo [!] detached HEAD - building the checkout as it is
  goto update-done
)
git -C "%REPO%" rev-parse --abbrev-ref "@{u}" >nul 2>nul
if errorlevel 1 (
  echo      %BRANCH% has no upstream - rebasing it onto origin/%BRANCH%
  git -C "%REPO%" fetch --quiet origin
  if errorlevel 1 goto update-failed
  git -C "%REPO%" rebase --autostash "origin/%BRANCH%"
) else (
  git -C "%REPO%" pull --rebase --autostash --quiet
)
if errorlevel 1 goto update-failed
echo      %BRANCH% is up to date with its remote
goto update-done
:update-failed
echo [!] could not update %BRANCH% from its remote - solve it (git status), then build again
goto fail
:update-done

echo [2/3] staging the D3D12 Agility SDK (shader model 6.6 on Windows 10)
cargo run --quiet --manifest-path "%REPO%\Cargo.toml" -p xtask -- agility
if errorlevel 1 (
  echo [!] xtask agility failed
  goto fail
)

echo [3/3] building victory.exe (profile diag)
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
