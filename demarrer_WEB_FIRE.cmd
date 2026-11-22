@echo off
rem Simulateur de feux de foret, a double-cliquer : garde la fenetre ouverte en cas derreur.
cd /d "%~dp0"
rem Un seul fichier a cliquer : le lanceur de lautre systeme (.sh) est masque.
attrib +h "%~dp0*.sh" >nul 2>nul
rem Code 75 : mis a jour, relancez-moi : on reprend l executable de la nouvelle version, puis on relance.
set WEB_FIRE_RELANCEUR=1
:encore
call scripts\binaires.cmd
if errorlevel 1 goto echec
bin\web_fire.exe %*
if %errorlevel%==75 (
  set WEB_FIRE_DEJA_MAJ=1
  goto encore
)
if not errorlevel 1 exit /b 0
:echec
echo.
echo [X] Le demarrage a echoue. La raison est ecrite au-dessus.
echo.
pause
