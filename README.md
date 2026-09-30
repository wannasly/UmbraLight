# UmbraLight

**Лёгкий нативный клиент sing-box для Windows** · версия **1.0.0**

UmbraLight работает в системном трее, а окно настроек запускает отдельным процессом по запросу. Приложение поддерживает системный прокси и TUN, подписки, правила маршрутизации и проверку задержки серверов.

## Возможности

- Подключение к серверам VLESS, VMess, Trojan, Shadowsocks и Hysteria2.
- Импорт ссылок и подписок, выбор активного сервера из трея или настроек.
- Системный прокси, TUN, DNS и правила маршрутизации.
- Пинг выбранного сервера и **Ping All** для проверки всех серверов за одно действие. Проверки выполняются группами по восемь; результаты отображаются в списке серверов.
- Автозапуск, журнал событий и диагностические инструменты.

## Сборка

Требуются Windows 10/11, [Rust и Cargo](https://rustup.rs/) с инструментами сборки MSVC.

Для обычной установки скачайте `UmbraLight-1.0.0-windows-x64-setup.exe` из [релиза v1.0.0](https://github.com/wannasly/UmbraLight/releases/tag/v1.0.0). Установщик кладёт приложение, sing-box и Wintun в папку пользователя, создаёт ярлык в меню «Пуск» и предлагает ярлык на рабочем столе. Профили при удалении приложения сохраняются.

```powershell
cargo build --release --workspace
```

После сборки в `target/release` находятся:

| Файл | Назначение |
| --- | --- |
| `UmbraLight.exe` | Основное приложение в трее |
| `UmbraLight-settings.exe` | Окно настроек |

При сборке из исходников разместите оба файла рядом. Для работы прокси нужен совместимый `sing-box.exe`: положите его рядом с приложением или в папку `resources` проекта. Для режима TUN потребуется `wintun.dll` рядом с приложением и запуск с правами администратора.

Запуск:

```powershell
.\target\release\UmbraLight.exe
```

Настройки открываются через иконку в трее. Данные профилей и настроек хранятся в `%APPDATA%\lightgui` для совместимости с предыдущими версиями приложения. Не публикуйте файлы `profiles.json` и `settings.json`: они могут содержать адреса и параметры подписок.

### Сборка установщика

Установите [Inno Setup 6](https://jrsoftware.org/isinfo.php) и выполните:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-installer.ps1
```

Скрипт соберёт release-версию, загрузит проверенные SHA-256 архивы [sing-box 1.13.14](https://github.com/SagerNet/sing-box/releases/tag/v1.13.14) и [Wintun 0.14.1](https://www.wintun.net/), затем создаст `target/installer/UmbraLight-1.0.0-windows-x64-setup.exe`. Лицензии компонентов входят в установку.

## Проверка

```powershell
cargo test --workspace
powershell -ExecutionPolicy Bypass -File .\scripts\test_runtime_smoke.ps1
```

Runtime smoke test использует отдельный временный каталог данных. Для него нужна собранная release-версия. Дополнительные сценарии находятся в `scripts/test_integration.ps1` и `scripts/test_edge_cases.ps1`.

## Структура проекта

| Путь | Содержимое |
| --- | --- |
| `crates/lightgui-core` | Модели, конфигурация sing-box, сеть, хранение данных и IPC |
| `crates/lightgui-tray` | Фоновый процесс и системный трей |
| `crates/lightgui-settings` | Нативное окно настроек Win32 |
| `docs` | Архитектура и технические заметки |

Внутренние имена Rust crates и каталог данных сохраняют префикс `lightgui` для совместимости; пользовательское название и исполняемые файлы — **UmbraLight**.
