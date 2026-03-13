@echo off
echo Building math-sonify release...
cargo build --release
if %ERRORLEVEL% neq 0 ( echo BUILD FAILED & pause & exit /b 1 )

echo.
echo Packaging...
if not exist dist mkdir dist
copy /Y "..\target\claude\release\math-sonify.exe" "dist\math-sonify.exe"
copy /Y "config.toml" "dist\config.toml"

echo.
echo Done! Distributable files are in math-sonify\dist\
echo   math-sonify.exe  - the application
echo   config.toml      - edit this to change defaults
echo.
pause
