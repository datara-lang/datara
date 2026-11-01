# PLAN_V152 — Datara/Forgen 1.5.2 «Multiplier»

База: 1.5.1 «Consolidator» (fdc2f28, tag v1.5.1). Статус-гейт 1.5.1:
311 сьютов 0 failed · fmt 0 · clippy 0 · parity 6/6 честная · size-gate ok ·
golden pipeline_demo один md5 на 3 тира · levels 4/4.

Позиционирование прежнее (язык-хаб: лучшее из Python/C/Rust/Fortran,
градуируемый контроль — НЕ Mojo-конкурент). Тема 1.5.2 — «умножение»:
дожать перенесённый DX-хвост 1.5.1, довести дистрибуцию до нулевого
порога входа и закрыть NatVar срез 3 (IO-поллер + честный async).

## N0. Гигиена релиза 1.5.1 `[ ]`

- [ ] CI на main: починить выпадающий сьют (наблюдение 2026-11-01:
  CI main/failure при зелёном теге — прогнать флейк-лист, зафиксировать
  повтор-правила как в bce).
- [ ] check/expr.rs (61.5KB, allow-list 1.5.1) — экстракция Cast-checker в
  `types/check/cast.rs`; снять allow-list запись.
- [ ] Ротация GitHub-токена (плановая, после серии пушей 1.5.1).

## N1. Дистрибуция: нулевой порог входа `[!]` (главная тема)

- [x] winget: PR microsoft/winget-pkgs#467946 (Datara.Datara 1.5.1,
  zip/portable x64, алиасы forgen/datara/dpm) — ДОБАВЛЕНО 2026-11-01;
  слежение за ревью-ботом (yaml-validation / pipeline-bot) до merge.
- [x] GitHub Release v1.5.1 — CI «Release & Packages» success, 17
  артефактов (zip, Setup.exe, deb, rpm, macOS×2, VSIX, npm-tgz, crate,
  whl, SHA256SUMS).
- [ ] crates.io: `cargo publish -p forgen` (нужен crates.io токен от
  владельца; dry-run уже зелёный в CI).
- [ ] npm: `npm publish` для datara-lang-datara tgz (нужен npm-токен).
- [ ] PyPI: `twine upload` whl (нужен PyPI-токен).
- [ ] Homebrew tap: репо datara-lang/homebrew-tap + формула из
  darwin/linux tar + SHA256SUMS (автогенерация формулы скриптом
  tools/gen_brew.rb).
- [ ] Scoop bucket: datara-lang/scoop-bucket + JSON-манифест (тот же zip).
- [ ] Chocolatey: пакет из Setup.exe (нужен choco-ключ; после winget-merge
  — перенос проверки целостности).
- [ ] VS Code Marketplace: publisher-аккаунт + `vsce publish` для
  datara-language-1.5.1.vsix.
- [ ] tools/release_checklist.py: одна команда проверяет — все ассеты
  релиза на месте, SHA256SUMS сходится, манифесты пакетных реестров
  обновлены (winget/homebrew/scoop) — «релиз без ручных шагов».

## N2. DX-хвост 1.5.1 → 1.5.2 `[ ]` (перенос из PLAN_V151 без потерь)

- [x] `forgen migrate 1.4→1.5` — ЗАКРЫТО 2026-11-01 (src/cli/migrate.rs,
  тесты test_migrate 3/3): let→val / let mut→mut (word-boundary,
  string/comment-aware), String→Str, одноарг println→out (скобочный
  матчинг, мультиарг не трогаем); отчёт {файл, строка, правило};
  `--dry-run` ничего не пишет; exit 1 на отсутствующий путь. Round-trip:
  мигрированный файл компилируется и даёт тот же вывод.
- [x] **Языковой мануал × 1.5-фичи** — ЗАКРЫТО 2026-11-01: секции
  §4.bis TSU (is/нарровинг через val, E-ANY-001), §12.bis Fibers & NatVar
  (spawn/join/run_until_done/каналы — канонические паттерны: join после
  run_until_done, recv с продюсером в спавн-фибере), §16.bis CLI
  (migrate/inspect/profile/dpm). Гейт tests/test_docs_coverage 4/4 —
  секции существуют И сниппеты компилируются и исполняются.
- [ ] `forgen watch` SLA-гейт: сохранение → диагностика < 200 мс;
  мутационный кейс (поломка → красная → фикс → зелёная).
- [ ] `forgen fmt` гейт идемпотентности: fmt(x)==fmt(fmt(x)) на examples +
  stdlib; `--check` в CI-джобе quality.
- [x] Resolver-hint — ЗАКРЫТО 2026-11-01 (resolver/visit.rs,
  тест test_resolver_hint): «Undefined symbol 'X'» сканирует packages/*/
  и выдаёт «add `use <pkg>` (found in package '<pkg>')» (fn/struct/
  record/behavior-декларации, word-boundary). Приоритет: похожее имя
  > пакетная подсказка.
- [ ] LSP × 1.5: completions fiber_*/chan-методов, hover `is`-сужения,
  E-ANY-001 в publishDiagnostics.
- [ ] CONTRIBUTING.md ревизия: чеклист из реального гейта (fmt/clippy/
  test, 60KB-гейт, смоуки) + «как добавить stdlib-модуль/тест-сьют».
- [x] docs/DPM_SCHEMA.md — ЗАКРЫТО 2026-11-01 (datara.toml/lock схема,
  $DATARA_HOME-раскладка, verify-семантика).
- [x] «Datara for C/Rust/Python developers» — ЗАКРЫТО 2026-11-01
  (docs/DATARA_FOR_C_RUST_PYTHON.md — таблицы без маркетинга).
- [x] tools/bridge_smoke.py — ЗАКРЫТО 2026-11-01: локальный прогон
  мостов одной командой (python-проба реально исполняется; c/js/rust —
  SKIP=покрыты сьютами).

## N3. NatVar срез 3: IO + честный async `[ ]`

- [ ] IO-поллер: Windows IOCP / Linux epoll в отдельном потоке; парковка
  фибера на сокете (EWOULDBLOCK → park → wake). API fiber_net_recv/send
  с таймаутом → Outcome. Тест: echo-сервер на фиберах, 100 соединений,
  1000 сообщений — ни одного блокирующего зависания.
- [ ] `rt_mutex_lock -> Outcome` (err="timeout") + `--detect-deadlock`
  (OFF по умолчанию): цикл в графе ожидания → диагностика с путём.
  Тест: AB-BA deadlock ловится за < 1 с.
- [ ] Честный бенчмарк vs tokio: echo 10k соединений (throughput, p99),
  100k спящих фиберов (память). Если tokio быстрее — задокументировать
  ЧТО и ПОЧЕМУ (правило честности). Артефакт bench_natvar/RESULTS.md.
- [ ] stdlib.async: FiberHandle-обёртки, join -> Outcome<T>, sleep_ms.

## N4. Экосистема-дожим `[ ]`

- [ ] stdlib.json замер vs C (10 МБ JSON): Datara vs C-эталон; если
  медленнее ×2 — C-ядро под капотом (артефакт bench_json/RESULTS.md).
- [ ] dpm polish: `dpm init` интерактивный (имя/версия/шаблон hello);
  сценарные ошибки «нет пакета/версии/подписи» с подсказками.
- [ ] tools/bridge_smoke.py: локальный прогон всех мостов одной командой
  (таблица статусов) — не зависящий от CI.
- [ ] WASM playground: docs/playground/ статическая страница с 3–5 демо
  (wasm-бекенд уже генерит .wasm/.js/.html); билд-скрипт одной командой.
- [ ] Онлайн-реестр Spark-пакетов: GitHub Pages (pages.yml уже есть) —
  index.json с подписанными пакетами + `dpm add` с URL.

## N5. Верификация и честность `[ ]`

- [ ] Паритет-матрица: расширить probes (формати-строки, дефолт-аргументы,
  closures в generic-функциях, `?` в методах behavior'ов) — цель 12+
  probe'ов на 3 тирах.
- [ ] Сохранить честность-правило: каждый «быстрее X» — с прогоном и
  артефактом; каждый fallback — FAIL тира в матрице.
- [ ] golden pipeline_demo остаётся CI-гейтом (3 тира, один md5).

## Порядок работ (зависимости)

N0 (CI-флейк, токен) → N1 (дистрибуция: crates/npm/PyPI/tap/scoop —
блокируется ТОЛЬКО токенами от владельца) → N2 (DX-хвост) → N3 (NatVar-3)
→ N4 → N5. Релиз 1.5.2 — тег после N0+главного из N1–N3.

## Критерии готовности 1.5.2 (DoD)

1. Гейт: cargo test --release 0 failed (все сьюты), fmt 0, clippy 0,
   size-gate ok, parity-матрица зелёная.
2. `forgen migrate` + мануальные секции + docs-гейты — в репо и CI.
3. Пакеты: crates.io/npm/PyPI — опубликованы (или явно задокументирован
   блокер-токен); winget-PR смержен; homebrew/scoop tap'ы живые.
4. NatVar-3: echo-сервер на фиберах 100 соединений без зависаний;
   deadlock-детектор ловит AB-BA; tokio-сравнение с артефактом.
5. Релиз v1.5.2: CI «Release & Packages» success, release_checklist.py —
   зелёный, тег + CHANGELOG_V152.md.
