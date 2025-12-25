# Rust Switcher

![Screenshot](assets/screenshots/overview.png)

## Русский

### Что это

Rust Switcher это утилита для Windows, которая:
- переключает раскладку
- умеет конвертировать выделенный текст между RU и EN раскладками
- умеет конвертировать последний введенный фрагмент по хоткею
- поддерживает автоконвертацию последнего слова при наборе (экспериментально)

### Ключевые возможности

- Convert Selection: конвертация выделения между RU и EN
- Convert Last Word: конвертация последнего введенного фрагмента по хоткею
- AutoConvert: автоконвертация последнего слова на пробеле и некоторых разделителях
- Input Journal: кольцевой буфер введенных символов для точной замены в редакторе
- Tracing: подробные логи в файл и консоль

### Скриншот

Файл: `assets/screenshots/overview.png`

Если хочешь другой путь, меняешь ссылку в начале README.

### Сборка и запуск

Требования:
- Rust nightly или stable (зависит от включенных фич)
- Windows 10 или 11

Команды:
- `cargo run`
- `cargo run --release`

### Конфигурация

- Параметры задержек и поведения живут в настройках приложения
- Логи включаются через `RUST_LOG`

### Профилирование

Рекомендуется `samply` на Windows для сэмплинг профилирования.
Пример:
- `samply record target\debug\rust-switcher.exe`

### Статус

Проект в активной разработке. Автоконвертация является экспериментальной и будет донастраиваться.

## English

### What is it

Rust Switcher is a Windows utility that:
- switches keyboard layouts
- converts selected text between RU and EN layouts
- converts the last typed chunk via hotkey
- supports experimental auto conversion on typing boundaries

### Key features

- Convert Selection: selection conversion between RU and EN
- Convert Last Word: hotkey driven conversion for the last typed chunk
- AutoConvert: auto conversion on space and some separators (experimental)
- Input Journal: ring buffer for reliable in place replacement
- Tracing: detailed logs to file and console

### Screenshot

File: `assets/screenshots/overview.png`

### Build and run

Requirements:
- Rust nightly or stable depending on enabled features
- Windows 10 or 11

Commands:
- `cargo run`
- `cargo run --release`

### Configuration

- Behavior and delays are configured via app settings
- Logging via `RUST_LOG`

### Profiling

Use `samply` on Windows for sampling profiling.
Example:
- `samply record target\debug\rust-switcher.exe`

### Status

Work in progress. AutoConvert is experimental and will be tuned.
