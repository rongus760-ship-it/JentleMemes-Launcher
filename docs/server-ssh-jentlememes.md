# Сервер jentlememes.ru — SSH и запуск бекенда

## Подключение по SSH

```bash
ssh -p 2222 rongus@jentlememes.ru
```

## Расположение бекенда

| Что | Путь |
|-----|------|
| Корень бекенда | `/var/www/jentlememes/` |

## Запуск после правок (перезапуск процессов)

На сервере:

```bash
cd /var/www/jentlememes
source venv/bin/activate
```

Остановить старые процессы:

```bash
pkill -f "python3 app.py"
pkill -f "python3 bot.py"
```

Запуск в фоне:

```bash
nohup python "bot.py" > output.log &
nohup python "app.py" > output.log &
```

> **Заметка:** оба процесса пишут в один и тот же `output.log` — последний запуск может перезаписать файл. При желании разведите логи, например: `bot.log` и `app.log`.

## Проверка, что процессы живы

```bash
pgrep -af "python.*app.py"
pgrep -af "python.*bot.py"
```

Или:

```bash
tail -f /var/www/jentlememes/output.log
```
