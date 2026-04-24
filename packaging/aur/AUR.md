# Публикация JentleMemes Launcher в AUR

В репозитории два варианта пакета (обычно в AUR держат **один** из них — чаще `-bin`, если есть готовые релизы).

| Каталог | Пакет | Смысл |
|--------|--------|--------|
| `jentlememes-launcher/` | `jentlememes-launcher` | Сборка из исходников (`npm` + `cargo`) по git-тегу `v$pkgver` |
| `jentlememes-launcher-bin/` | `jentlememes-launcher-bin` | Скачивает готовый **`*.pkg.tar.zst`** с GitHub Releases и пересобирает его в пакет `-bin` (ставится через `pacman` / `yay` как обычно) |

Перед отправкой в AUR замените строку `# Maintainer: ...` в `PKGBUILD` на своё имя и email (формат AUR).

### Формат после `makepkg`: `*.pkg.tar.zst`

Это **ожидаемый результат**: `makepkg` всегда собирает установочный пакет Arch в виде `pkgname-pkgver-pkgrel-arch.pkg.tar.zst`. В `PKGBUILD` **ничего менять не нужно** — расширение задаёт не рецепт, а `makepkg`/`pacman`.

- В массиве **`source=`** перечислены **исходники** с GitHub (`.tar.gz`, готовый `.pkg.tar.zst` для `-bin`, `.desktop` и т.д.), а не локальный итог `makepkg` для AUR.
- В **AUR** в git кладут только **`PKGBUILD`** и **`.SRCINFO`**. Сам файл `*.pkg.tar.zst` на AUR **не загружают** — его собирают у себя `yay`/`paru` и другие пользователи.
- Отдельная история: **ручной** архив бинарника через `tar | zstd` из README — это для **админки сайта**, не замена формату `makepkg`.

### Выложить готовый `*.pkg.tar.zst` для ручной установки (без AUR)

Так можно: собрали `makepkg -f`, загрузили файл на сайт / в GitHub Releases — пользователь качает и ставит сам.

```bash
sudo pacman -U ./jentlememes-launcher-2.0.0-1-x86_64.pkg.tar.zst
# или с полным путём к скачанному файлу
```

У пользователя должны быть установлены **зависимости** из `depends=` в `PKGBUILD` (`gtk3`, `webkit2gtk-4.1`, `libayatana-appindicator` и т.д.) — `pacman` подскажет, если чего-то не хватает.

Нюансы:

- Пакет **не обновляется сам** через `pacman -Syu`, пока вы не ведёте свой репозиторий или пользователь снова не скачает новый `.pkg.tar.zst` и не выполнит `pacman -U`.
- Для доверия к пакету в продвинутых сценариях можно подписывать пакет и добавлять ключ в `pacman`; для личного/небольшого проекта часто обходятся без подписи (пользователь явно указывает файл).

### Автоматическая установка и обновления (Arch)

**Вариант A — AUR и помощник (`yay` / `paru`).** Пакет в AUR один раз публикуете вы; пользователь ставит и обновляет одной командой (помощник сам качает `PKGBUILD`, собирает или подтягивает бинарный вариант по рецепту):

```bash
yay -S jentlememes-launcher-bin
# далее обновления вместе с системой:
yay -Syu
```

Для пользователя это максимально «автоматично» среди вариантов без своего зеркала.

**Вариант B — свой репозиторий для `pacman`.** Вы собираете `*.pkg.tar.zst`, кладёте на HTTPS-хостинг вместе с базой репозитория (создаётся утилитой **`repo-add`** из пакета `pacman-contrib`):

```bash
repo-add jentlememes.db.tar.gz jentlememes-launcher-2.0.0-1-x86_64.pkg.tar.zst
# загрузите на сервер: jentlememes.db.tar.gz, jentlememes.files.tar.gz (если создались) и сами .pkg.tar.zst
```

У пользователя в `/etc/pacman.conf` (один раз):

```ini
[jentlememes]
SigLevel = Optional TrustAll
Server = https://ваш.домен/pacman/$arch
```

Дальше обычный **`sudo pacman -Sy jentlememes-launcher`** и обновления через **`sudo pacman -Syu`** — как с любым официальным репозиторием. При желании настройте подпись пакетов (`gpg`, `SigLevel = Required` и ключ в `pacman-key`).

**Вариант C — обновление из самого лаунчера.** На сайте уже может быть логика проверки версии (см. код обновлений); это не заменяет `pacman`, но даёт «скачал и заменил бинарник» без участия диспетчера пакетов Arch.

---

## 1. Подготовка релиза на GitHub

1. Убедитесь, что версия совпадает во всех местах: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src/version.ts`, `src-tauri/src/core/mrpack.rs`, установщик в `installer/`.
2. Создайте git-тег: `git tag v2.0.0` и отправьте: `git push origin v2.0.0`.
3. На GitHub создайте **Release** с тем же тегом и приложите артефакты Linux. Для **`jentlememes-launcher-bin`** в релиз нужно выложить **уже собранный pacman-пакет** из `packaging/aur/jentlememes-launcher/PKGBUILD`:  
   `jentlememes-launcher-${pkgver}-${pkgrel}-x86_64.pkg.tar.zst`  
   (соберите `makepkg -f` в каталоге с этим `PKGBUILD`, затем загрузите получившийся файл в assets релиза `v${pkgver}`). В `-bin`/`PKGBUILD` имя задаётся через `_upstream_pkg` и **`_upstream_pkgrel`** (он должен совпадать с `pkgrel` того пакета, который вы выложили; это не обязательно равно `pkgrel` самого `-bin` в AUR).

Без этого файла в Release пакет **`jentlememes-launcher-bin`** не соберётся (404 в `source=`).

Дополнительно по желанию: **AppImage** и прочие форматы с `tauri build` — для пользователей не-Arch; на `-bin` PKGBUILD они больше не завязаны.

Для пакета **из исходников** достаточно тега: архив `v${pkgver}.tar.gz` GitHub создаёт автоматически. В `source=` второй файл — `jentlememes-launcher.desktop` с **raw** URL того же тега; он должен уже лежать в репозитории по пути `packaging/aur/jentlememes-launcher.desktop`.

---

## 2. Аккаунт и ключ для AUR

1. Зарегистрируйтесь на [https://aur.archlinux.org](https://aur.archlinux.org).
2. Добавьте SSH public key в профиль (как для обычного SSH-доступа к `aur@aur.archlinux.org`).

---

## 3. Первичная выгрузка пакета

Выберите **один** вариант (или два разных имени пакета в AUR, если нужны оба).

```bash
# Пример: только бинарный пакет
mkdir jentlememes-launcher-bin
cp /path/to/repo/packaging/aur/jentlememes-launcher-bin/PKGBUILD jentlememes-launcher-bin/
cd jentlememes-launcher-bin
updpkgsums   # sha256 для .pkg.tar.zst с GitHub (релиз уже должен содержать артефакт)
makepkg -f
```

Проверьте установку: `pacman -U ./jentlememes-launcher-bin-*.pkg.tar.zst`.

Инициализация git-репозитория на стороне AUR:

```bash
cd jentlememes-launcher-bin
git init
git remote add aur ssh://aur@aur.archlinux.org/jentlememes-launcher-bin.git
git add PKGBUILD .SRCINFO
git commit -m "initial release"
git push aur master
```

Генерация `.SRCINFO` (обязательно для AUR):

```bash
makepkg --printsrcinfo > .SRCINFO
```

Добавьте `.SRCINFO` в коммит перед `git push`.

---

## 4. Обновление версии

1. В GitHub: новый тег `vX.Y.Z`, в релиз — `jentlememes-launcher-X.Y.Z-*-x86_64.pkg.tar.zst` (для `-bin`).
2. В локальной копии AUR-репозитория отредактируйте `PKGBUILD`: `pkgver`, сбросьте или увеличьте `pkgrel` по правилам AUR.
3. `updpkgsums`, `makepkg --printsrcinfo > .SRCINFO`, `makepkg -f`, затем `git add` / `commit` / `push` в `aur`.

---

## 5. Замечания

- Для `-bin` имя `*.pkg.tar.zst` в Release должно совпадать с `_upstream_pkg` / `_upstream_pkgrel` в `PKGBUILD`. Если переименовываете артефакт — поправьте шаблон в `source=`.
- Если лицензия проекта иная, обновите массив `license=` в `PKGBUILD` и при необходимости установите файлы лицензий в `package()` через `install -Dm644`.
- Сборка из исходников долгая и тянет `nodejs`/`rust`; многие мейнтейнеры предпочитают `-bin`.
- Проверка правил: [AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines).

### Частая ошибка: первая строка PKGBUILD

Строка maintainer **обязана** быть **комментарием** (с `#` в начале). Иначе `makepkg` / `yay` выдают синтаксическую ошибку на строке 1:

```text
# Maintainer: Имя <email@example.com>
```

Неправильно: `Maintainer: ...` без `#`.
