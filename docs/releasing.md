# Релиз

## Подготовка

1. Создайте секрет репозитория `CARGO_REGISTRY_TOKEN` с токеном crates.io.

## Что происходит при каждом вливе в `main`

- Workflow **Bump patch version** автоматически повышает patch-версию в `Cargo.toml`,
  создаёт коммит `Bump version to vX.Y.Z` и тег `vX.Y.Z`.
- Публикация в crates.io выполняется автоматически (при наличии секрета
  `CARGO_REGISTRY_TOKEN`).
- GitHub Release создаётся автоматически.

## Выпуск релиза

1. Откройте GitHub Actions и запустите workflow **Release** вручную.
2. В input `bump` выберите тип увеличения версии (`patch`, `minor`, `major`).
3. Дождитесь завершения workflow.

## Результат

- Создан новый тег `vX.Y.Z`.
- Создан GitHub Release с прикреплённым Windows asset.
- Крат опубликован в crates.io.
