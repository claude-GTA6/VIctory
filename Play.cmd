@echo off
setlocal EnableExtensions
rem Starts the title through this build. You need your OWN copy of the game (your own dump of the Xbox Series
rem version you own). No game files come with this.
rem
rem First run: drag the title's .exe (or its folder) onto this file, or paste the path when asked.
rem The path is remembered in %LOCALAPPDATA%\victory\play-game-path.txt (delete that file to choose again).
rem The console window shows the log; close the game window to stop.

set "HERE=%~dp0"
set "VICTORY="
if exist "%HERE%target\release\victory.exe" set "VICTORY=%HERE%target\release\victory.exe"
if not defined VICTORY if exist "%HERE%victory.exe" set "VICTORY=%HERE%victory.exe"
if not defined VICTORY (
  echo victory.exe not found. Build it first, in this folder:
  echo     cargo build --release -p victory
  goto :end
)

set "SAVED=%LOCALAPPDATA%\victory\play-game-path.txt"
set "GAME=%~1"
if not defined GAME if exist "%SAVED%" set /p GAME=<"%SAVED%"
if not defined GAME (
  echo Where is YOUR copy of the game? Paste the path of its .exe, or of the folder that holds it:
  set /p GAME=^>
)
if not defined GAME goto :nogame
set "GAME=%GAME:"=%"

rem A folder: use the only .exe at its top level.
if exist "%GAME%\" (
  set "FOUND="
  set "COUNT=0"
  for %%F in ("%GAME%\*.exe") do (
    set "FOUND=%%~fF"
    set /a COUNT+=1
  )
  call :pickfolder
)
if not exist "%GAME%" goto :nogame
if not exist "%LOCALAPPDATA%\victory" mkdir "%LOCALAPPDATA%\victory"
> "%SAVED%" echo %GAME%

rem ---- Settings ----
rem Relay is on by default in this code (fxceless's setting); we have not finished testing it on our hardware. Delete this line to use it.
set VICTORY_RELAY=0

echo Starting: "%GAME%"
rem High priority: background programs otherwise take CPU time the title needs.
start "victory" /high /wait "%VICTORY%" run "%GAME%" --execute
goto :end

:pickfolder
if "%COUNT%"=="1" (set "GAME=%FOUND%" & exit /b)
echo That folder has %COUNT% .exe files at its top level. Drag the title's .exe itself onto this file instead.
set "GAME="
exit /b

:nogame
echo No game found at "%GAME%". Point this at your own copy of the title's .exe.
if exist "%SAVED%" del "%SAVED%"

:end
pause
