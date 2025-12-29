# Rust Switcher

![Screenshot](assets/screenshots/overview.png)

Rust Switcher это утилита для Windows для конвертации текста между RU и EN раскладками и автоконвертации последнего слова при наборе.

## Возможности

- Конвертация выделенного текста RU EN
- Конвертация последнего введенного слова по хоткею
- Автоконвертация последнего слова при наборе (можно поставить на паузу)
- Tray icon и настройки

## Системные требования

- Windows 11
- Rust nightly (см. `rust-toolchain.toml`)
- MSVC toolchain (Visual Studio 2022 Build Tools)

## Установка и запуск

Сборка:
- `cargo +nightly build`

Запуск:
- `cargo +nightly run`

Release сборка:
- `cargo +nightly build --release --locked`

## Разработка

### Быстрый цикл через Bacon

Проект содержит `bacon.toml` и строгий раннер `scripts/bacon_strict.ps1`.

Установка:
- `cargo install bacon`

Запуск:
- `bacon`

Горячие клавиши Bacon:
- `d` dev
- `r` release
- `t` tests
- `p` dushnota

### Проверки перед PR

- `cargo +nightly fmt --check`
- `cargo +nightly clippy -- -D warnings`
- `cargo +nightly test`

## Конфигурация

Настройки меняются в GUI. Логирование контролируется feature флагом и переменной `RUST_LOG`.
