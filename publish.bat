@echo off
rem Отправляет проект на GitHub. После push сборка стартует сама.
rem Использование: publish.bat  или  publish.bat --force
cd /d "%~dp0"
if not exist .git git init
git add -A
git commit -m "ps5emu: каркас, лаунчер и автосборка"
git branch -M main
git remote remove origin 2>nul
git remote add origin https://github.com/kochvalds/ps5emu
if "%1"=="--force" (git push -u origin main --force) else (git push -u origin main)
echo Готово. Сборку смотрите на https://github.com/kochvalds/ps5emu/actions
