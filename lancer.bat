@echo off
cd /d "%~dp0"
rem Code 75 : mis a jour, relancez-moi : on reprend l executable, puis on relance.
set WEB_FIRE_RELANCEUR=1
:boucle
call binaires.cmd || goto fin
bin\web_fire.exe %*
if %errorlevel%==75 goto boucle
:fin
pause
