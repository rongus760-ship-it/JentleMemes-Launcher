#!/usr/bin/env bash
# Пример: залить обновлённый Flask (app.py) на сервер.
# Скопируйте в deploy-web-backend.sh, задайте переменные и выполните вручную.
#
#   export REMOTE_USER=deploy
#   export REMOTE_HOST=jentlememes.ru
#   export REMOTE_DIR=/var/www/jentlememes-social   # каталог, где лежит app.py
#   bash scripts/deploy-web-backend.example.sh

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/web-backend/app.py"
: "${REMOTE_USER:?Задайте REMOTE_USER}"
: "${REMOTE_HOST:?Задайте REMOTE_HOST}"
: "${REMOTE_DIR:?Задайте REMOTE_DIR (путь к каталогу с app.py)}"

echo "→ $REMOTE_USER@$REMOTE_HOST:$REMOTE_DIR/app.py"
scp "$SRC" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_DIR}/app.py"
echo "На сервере перезапустите сервис (например: systemctl restart jentlememes-api)."
