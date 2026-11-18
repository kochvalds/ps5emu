#!/usr/bin/env bash
# Отправляет проект на GitHub. После push сборка стартует сама (.github/workflows/release.yml).
# Использование: ./publish.sh          (обычный push)
#                ./publish.sh --force  (если в репозитории уже есть чужие коммиты)
set -e
cd "$(dirname "$0")"
REPO=https://github.com/kochvalds/ps5emu

[ -d .git ] || git init
git add -A
git commit -m "ps5emu: каркас, лаунчер и автосборка" || true
git branch -M main
git remote remove origin 2>/dev/null || true
git remote add origin "$REPO"
if [ "${1:-}" = "--force" ]; then
  git push -u origin main --force
else
  git push -u origin main
fi
echo "Готово. Сборку смотрите на https://github.com/kochvalds/ps5emu/actions"
