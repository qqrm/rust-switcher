# Релиз

## Подготовка

1. Создайте секрет репозитория `CARGO_REGISTRY_TOKEN` с токеном crates.io.

## Выпуск релиза

1. Откройте GitHub Actions и запустите workflow **Release** вручную.
2. В input `bump` выберите тип увеличения версии (`patch`, `minor`, `major`).
3. Дождитесь завершения workflow.

## Результат

- Создан новый тег `vX.Y.Z`.
- Создан GitHub Release с прикреплённым Windows asset.
- Крат опубликован в crates.io.
