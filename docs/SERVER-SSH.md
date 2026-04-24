# Сервер jentlememes.ru — SSH и перезапуск бэкенда

## Подключение

```bash
ssh -p 2222 rongus@jentlememes.ru
```

## Каталог бэкенда

```text
/var/www/jentlememes/
```

Рекомендуется виртуальное окружение Python в этом каталоге: `venv/`.

## Перезапуск `app.py` и `bot.py`

После обновления кода на сервере:

```bash
cd /var/www/jentlememes
source venv/bin/activate

# Остановить старые процессы
pkill -f "python3 app.py"
pkill -f "python3 bot.py"
# или при запуске через `python` без цифры 3:
pkill -f "python app.py"
pkill -f "python bot.py"

# Запуск в фоне с логами
nohup python "bot.py" > bot_output.log 2>&1 &
nohup python "app.py" > output.log 2>&1 &
```

Проверка:

```bash
tail -f output.log
```

## Деплой из репозитория

В репозитории лаунчера каталог `web-backend/` — эталонный код API (`app.py`, `requirements.txt`, `bot.py`). Скопируйте файлы на сервер (rsync/scp), затем:

```bash
cd /var/www/jentlememes
source venv/bin/activate
pip install -r requirements.txt
```

Если на сервере уже есть свои `app.py` / `bot.py`, сделайте резервную копию перед заменой и при необходимости объедините изменения.

## Загрузка обновлений лаунчера (413 Request Entity Too Large)

1. **Flask** — в `.env` можно задать `FLASK_MAX_UPLOAD_MB=1024` (по умолчанию в коде уже 1024 МБ).
2. **nginx** — в `server { ... }` для `jentlememes.ru` должно быть не меньше, чем у Flask, например:

```nginx
client_max_body_size 1024m;
```

После правки: `sudo nginx -t && sudo systemctl reload nginx`.
