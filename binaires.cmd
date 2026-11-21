@echo off
rem Met l'executable du simulateur dans bin\ et le moteur WebAssembly dans web\wasm\ (appele par lancer.bat).
rem Avec cargo sur le poste : compile ici, toujours a jour avec les sources.
rem Sinon : telecharge depuis GitHub (publication "binaires", compilee par la CI a chaque
rem envoi sur main), une fois par version : bin\commit dit de quelle version il vient.
setlocal
set DEPOT=https://github.com/pilou33620/web_fire/releases/download/binaires
if not exist bin mkdir bin
if not exist web\wasm mkdir web\wasm
set ICI=
for /f %%c in ('git rev-parse HEAD 2^>nul') do set ICI=%%c
call :wasm
where cargo >nul 2>nul
if not errorlevel 1 (
  cargo build --release -p web_fire --bins || exit /b 1
  copy /y target\release\*.exe bin\ >nul 2>nul
  exit /b 0
)
set LA=
if exist bin\commit set /p LA=<bin\commit
if exist bin\web_fire.exe if defined ICI if "%ICI%"=="%LA%" exit /b 0
echo   Telechargement du simulateur (windows-x86_64)...
call :telecharger web_fire-windows-x86_64.zip bin && exit /b 0
if exist bin\web_fire.exe (
  echo   [!] Telechargement impossible : on garde l'executable deja present.
  exit /b 0
)
echo [X] Telechargement du simulateur impossible.
echo     Installer Rust (https://rustup.rs) puis relancer : il sera compile ici.
exit /b 1

rem Le moteur dans le navigateur : facultatif (sans lui, la page passe par le serveur).
:wasm
set LW=
if exist web\wasm\commit set /p LW=<web\wasm\commit
if exist web\wasm\feu_wasm_bg.wasm if defined ICI if "%ICI%"=="%LW%" exit /b 0
echo   Telechargement du moteur pour le navigateur (WebAssembly)...
call :telecharger feu-wasm.tar.gz web\wasm || echo   [!] Moteur WebAssembly non telecharge : la page passera par le serveur.
exit /b 0

rem telecharger ARCHIVE DOSSIER : l'archive doit avoir l'empreinte SHA-256 publiee avec elle
:telecharger
curl -fsSL -o bin\.archive %DEPOT%/%1 || goto echec
curl -fsSL -o bin\.empreinte %DEPOT%/%1.sha256 || goto echec
set ATTENDU=
set /p ATTENDU=<bin\.empreinte
set RECU=
for /f "skip=1 delims=" %%h in ('certutil -hashfile bin\.archive SHA256') do if not defined RECU set RECU=%%h
set RECU=%RECU: =%
if /i not "%RECU%"=="%ATTENDU%" (
  echo   [!] Empreinte de l'archive incorrecte : archive refusee.
  goto echec
)
tar -xf bin\.archive -C %2 || goto echec
del bin\.archive bin\.empreinte >nul 2>nul
exit /b 0
:echec
del bin\.archive bin\.empreinte >nul 2>nul
exit /b 1
