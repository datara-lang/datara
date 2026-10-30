# DATARA ROADMAP — v1.5.1 «CONSOLIDATOR»

> Файл-план работ: 1.5.0 → 1.5.1. Исполнительный документ; спутник:
> [PLAN_V150.md](PLAN_V150.md) (итоги и DoD-сводка §3.5).
> Главный принцип 1.5.1: **ноль ломающих изменений**. Обратная совместимость
> с тега v1.5.0 объявлена обязательной (VISION §12) — только аддитивные
> изменения, стабилизация, DX и экосистема. Новый компиляторный «флагман» —
> не в этом релизе.
>
> Правило честности: не подгонять код под тесты; каждый пункт закрывается
> воспроизводимой проверкой (команда + вывод + exit-код). Гейт каждой фазы:
> `cargo build --release` 0 warnings, `cargo clippy --release --all-targets`
> 0 warnings, `cargo test --release` 0 failed **×3 прогона подряд**
> (анти-флейк), check-смоук examples 29/29 + showcase, `forgen test
> stdlib_tests` 60+, game_loop CHECKSUM:-273743660 на трёх бэкендах.

Легенда: `[ ]` не начато · `[~]` в работе · `[x]` завершено с проверкой ·
`[!]` заблокировано (нужно от человека) · `[-]` отменено (с причиной).

---

## 0. Зачем 1.5.1 (позиционирование)

1.5.0 закрыл ядро релиза (DoD пп. 1–8, 12). Остатки — инфраструктурные:
NatVar-срез 2, экосистема (3/5 пакетов, подпись), DX (migrate, dpm init,
bridge check), стабильность (флейки, реестр багов §5 v150). 1.5.1 — релиз
«первого пользователя»: цель, чтобы разработчик, пришедший извне, смог за
вечер поставить язык, пройти туториал, поставить пакеты из реестра и не
споткнуться ни об один известный тихий баг.

Целевая дата: 4–6 недель работы. Релиз-машина (push/тег) — от владельца.

---

## N0. Гигиена релиза и репозитория `[ ]`

- [x] tmp_*-файлы в корне — ЗАКРЫТО 2026-10-28: план-доказательства
  переехали в `evidence/` (m4/ — 21 стресс-отчёт, m5_deref/ — @arc-замер,
  m6/ — dataflow-пробы, natvar/, probes/ — .dtr-репродукторы багов; README
  с картой). Сгенерированные деревья (tmp_ab_200k — 120+ модулей) и
  package-consumers удалены — воспроизводятся tools/gen_stress.py и
  `dpm publish`-флоу. Бинарники в evidence не коммитятся, .gitignore
  держит свежий мусор. Коммит 868d295.
- [x] Стабилизировать тайминг-флейк test_bce_array_sum_microbenchmark —
  ЗАКРЫТО 2026-10-28 (двумя фиксами): test_v120_phase8 — min-of-11 и порог
  1.15x; test_phase12 — повтор-измерение до 3 попыток при ratio > 1.10
  (gate — инвариант компиляционного паритета, не speed-claim; оба цикла
  дают идентичный машинный паттерн). Критерий: 6 подряд прогонов
  test_phase12_bce зелёные — выполнено; полный сьют 1509/317 зелёный.
- [ ] Русские комментарии в .dtr читаются кракозябрами при некоторых путях
  (v150 §5): фикс read-path на строгий UTF-8 с понятной диагностикой (уже
  есть UTF-8 guard для fmt/lint — расширить на все пути чтения исходников);
  ревизия examples с кириллицей. Критерий: `forgen check` + `run` всех
  файлов с кириллическими комментариями → корректный вывод байт-в-байт
  против эталона.
- [ ] `dpm clean` (команда уже существует) — расширить на `.forgen_cache/`,
  `.forgen_profile/`, временные exe рядом с таргетом; задокументировать.
  Критерий: повторный `build` после `clean` succeeds; `clean` не трогает
  исходники; дважды подряд `clean` — без ошибок.

## N1. Реестр багов v150 — добить открытые `[ ]` `[~]` `[ ]`

- [ ] `str_scalar_at` принимает байтовое смещение (v150 §5): добавить
  `datara_rt_str_scalar_at_cp` с cp→byte-трансляцией, `str_scalar_at`
  переименовать в `str_scalar_at_byte` (алиас оставить), stdlib.text
  переключить на cp-вариант. Тест: "χαίρετε"[1] == "α" на обоих тирах;
  регресс на байтовом пути.
- [ ] `List.new(n)` / `Outcome<T>` без аргументов (остаток серии Map.new):
  static-ctors по образцу Map.new — checker (current_assign_target) +
  lowering-intercept + infer + resolver. Тест: `List.new(5)` предзаполнен
  нулями длины 5; `val r = Outcome<Int>`… — fail-loud использование до
  присваивания; регресс-тесты на обоих тирах.
- [~] Вывод типа результата произвольного stdlib-метода без аннотации
  (остаток v150 §5): расширить infer-таблицу class-receiver method calls
  (is_ok/unwrap/get/pad_left и пр. уже частично). Критерий: `val s =
  StringUtils.split(...)` и 3 новых кейса из stdlib.web.ssr/dpm — тип
  выводится без аннотации; тест в test_list_str_index.rs.
- [ ] Аудит «тихих падений» после M7/M10-правок: сгенерировать матрицу
  программ (generic + val + fiber + return-? + пакетные импорты) и прогнать
  дифф stdout Cranelift/JIT/LLVM — ноль расхождений. Скрипт
  tools/parity_matrix.py + отчёт evidence/parity_matrix.json.
- [x] Каскад кастов (N1 #4, ЗАКРЫТО 2026-10-30): пакетный consumer resultx
  вскрыл цепочку из 6 багов. (1) DCE collect_used_values не учитывал
  операнды Cast/VolatileLoad/VolatileStore/InlineAsm — def каста удалялся
  (E0901 после intraproc; Bool→Int не сворачивается const-fold'ом —
  константы в bool_constants, а не int_constants). (2) inline
  substitute_operands фиксил только dest — stale pre-inline id (E0901 после
  inline). (3) infer_expr_datara_type не знал Expr::Cast → вложенный каст
  `(x as Float) as Int` получал from_ty="Int" на f64-носителе: паника
  Cranelift ISLE x64_lea + llc-ошибка `or i64` на double. (4) У LLVM-эмиттера
  НЕ БЫЛО ветки Inst::Cast — llc падал на любом выжившем касте, а `--llvm`
  молча откатывался на Cranelift. (5) lowering function_return_types не имел
  str_scalar_at/_cp → DMIR Call ty="Int" → `ret i64` в ptr-функции. (6)
  emit_call-маппинг без str_scalar_at_cp/str_substring_cp → undefined
  symbol. Плюс (7) tools/parity_matrix.py считал fallback за OK — теперь
  "Falling back" = провал тира. Регресс-тесты: test_dce_bool_cast_operand_
  survives, test_inline_cast_operand_substituted, test_nested_cast_
  roundtrip_all_tiers; probe evidence/probes/cast_bool_var.dtr. Гейт:
  parity 6/6 (честная), main5 3 бекенда байт-в-байт.
- [x] Каскад gcd/abs (N1 #5, ЗАКРЫТО 2026-10-31): честная parity-матрица
  поймала LLVM-fallback на str_scalar probe, а consumer main_all — дивергенцию
  gcd (LLVM 36/4 вместо 12). Два бага: (1) LLVM emit_call угонял ИМЯ
  пользовательской функции в builtin — пакетный `fn abs(Int) -> Int`
  становился `datara_rt_math_abs(double)` при `call i64 …(i64)`:
  декларация/вызов не совпадали, opt -O3 -mcpu=native (znver4) miscompile;
  llc -O0 давал 36, llc -O2 12, пайплайн 4 — классика UB. Фикс: guard
  `module.functions.contains_key(func)` — user-fn всегда побеждает builtin-
  мапу; плюс type-aware Int-abs → datara_rt_math_abs_int (i64-носитель).
  (2) lowering function_return_types не знал str_scalar_at/_cp → DMIR Call
  ty="Int" → `ret i64` в ptr-функции (подпункт каскада кастов, закрыт там же).
  Методика: FORGEN_KEEP_LLVM=1 + ручной opt/llc/link -O0 vs -O2 + симуляция
  SSA на бумаге. Регресс: test_llvm_int_abs_loop_parity. Гейт: main_all
  (5 пакетов, 13 строк) байт-в-байт на Cranelift/JIT/LLVM.

## N2. NatVar срез 2: многопоточный планировщик + IO-поллер `[ ]`

Фундамент готов: chase_lev work-stealing deque (src/runtime/scheduler/
chase_lev.rs, 11 юнит-тестов), NUMA-топология, C-фиберы с таймер-колесом.

- [x] Многопоточный планировщик — ЗАКРЫТО 2026-10-28 (datara_natvar_mt.c):
  N воркер-потоков (CreateThread/pthread), per-worker ready-очереди +
  глобальная + кража round-robin; MT-API: fiber_mt_start(n)/mt_spawn(f,arg)/
  mt_join(h)/mt_wait_idle()/mt_done_count()/mt_spawned(). Ключевой контракт:
  fiber == one-shot задача (тело выполняется на подобравшем воркере);
  кооперативные yield внутри тела — домен срез-1 планировщика. Тесты
  tests/test_natvar_mt.rs 3/3: 10k задач на 8 воркерах — 10000/10000 слотов
  корректны (single-writer паттерн; кросс-воркер инкременты общего счётчика
  — data race по семантике, атомики — срез 3), два раунда, счётчик spawned.
  Бенчмарк: 100k фиберов — 76 мс (~755 нс/задача spawn+run+verify),
  evidence/natvar/MT_RESULTS.md (честно: tokio-референс не прогонялся).
  Регистрации: declare_ext/jit (все datara_rt_* алиасы — без них JIT не
  резолвит)/llvm; resolver+prelude. Ловушка Cranelift: ЛЮБАЯ
  declare_function(Import) попадает в obj как undefined symbol — декларации
  без реализации в datara_runtime.lib дают LNK2001 (было: stray
  datara_rt_fiber_mt_now).
- [ ] IO-поллер: Windows IOCP / Linux epoll в отдельном потоке; парковка
  фибера на сокете (EWOULDBLOCK → park → wake). API: `fiber_net_recv/
  fiber_net_send` с таймаутом → Outcome. Тест: echo-сервер на фиберах, 100
  соединений, 1000 сообщений — ни одного блокирующего зависания.
- [ ] `rt_mutex_lock -> Outcome` (err="timeout") + `--detect-deadlock`
  (OFF по умолчанию): цикл в графе ожидания фиберов → диагностика с путём.
  Тест: классический AB-BA deadlock ловится за < 1 с.
- [ ] Честный бенчмарк vs tokio-референс на той же машине: echo 10k
  соединений (throughput, p99), 100k спящих фиберов (память). Если tokio
  быстрее — задокументировать ЧТО и ПОЧЕМУ (правило честности). Артефакт:
  bench_natvar/RESULTS.md + runner-скрипт.
- [ ] stdlib.async: FiberHandle-обёртки, join -> Outcome<T>, sleep_ms —
  публичная поверхность поверх builtins. Тест в stdlib_tests.

## N3. Экосистема: пакеты, подпись, bridges `[ ]`

- [x] Два новых пакета в реестр: **resultx** (Outcome/Maybe-хелперы —
  typed-Int-вариант: ограничение чекера на поля generic-параметров в bare-
  generic телах задокументировано в lib.dtr) и **timex** (format_ms/
  format_count/elapsed_since) — ЗАКРЫТО 2026-10-30. Путь: fixtures →
  `dpm publish` → consumer `use resultx`/`use timex` → run на 3 бэкендах.
  Consumer-верификация: main5 (6 проверок) + main_all (все 5 пакетов
  реестра в одной программе, 13 строк вывода) — байт-в-байт на
  Cranelift/JIT/LLVM; реестр 5/5 (mathx/strx/jsonx/resultx/timex).
  Побочно: контекст-адопшн v1.5.0 (`mut r: Outcome<Int> = Outcome.err("…")`)
  верифицирован в consumer; найден каскад багов N1 #4/#5.
- [x] Локальная Ed25519-подпись при publish — ЗАКРЫТО 2026-10-31: ключ
  datara-core-2026-v2 сгенерирован (`dpm keygen datara-core-2026-v2`,
  sk в $DATARA_HOME/keys + бэкап D:\DATARA\sparks), pk доверен
  (`dpm trust <pk>` → $DATARA_HOME/trust/<pk>.pk). Publish подписывает
  bundle detached-файлом `<bundle>.sig` (формат `ed25519:<pk>:<sig>`,
  сам-описывающий) в dist/ и CAS; подпись опциональна (unsigned-publish
  легален). `dpm verify` для каждого залоченного пакета: Unsigned → как
  раньше; Valid → «ed25519 signature verified (pk …)»; ключ вне
  trust-store → WARNING; доверенный ключ + битая подпись → **Mismatch**
  (эскалация при совпадающем дайджесте). Trust-dir деривация:
  <store>/../trust (портативные инсталляции и тесты). Живая верификация:
  5/5 пакетов подписаны и verified; тампер pk-замена → WARNING, битая sig
  при доверенном pk → FAIL; restore → 5/5 verified. Тесты
  tests/test_dpm_signing.rs 3/3 (keypair-roundtrip, все ветки sig-status,
  эскалация в registry.verify). Онлайн-реестр/GitHub Pages — отдельно
  (нужен push, N6).
- [ ] `forgen bridge check`: статическая сверка bridge-деклараций с
  заголовками/стабами (аргументы/типы/арность; наследие плана B1). Тест:
  корректный мост проходит, мост с несовпадающей сигнатурой даёт
  E-BRIDGE-диагностику с точным местом.
- [ ] Python-мост hot-path аудит: py.bind_buffer — замер 10 МБ List<Float>
  → numpy без копирования (до/после, `DATARA_BRIDGE_TRACE=1` счётчик
  копий). Если копия есть — устранить. Артефакт: bench_python/RESULTS.md.
- [ ] dpm polish: `dpm init` интерактивный (имя/версия/шаблон hello), схема
  datara.toml — docs/DPM_SCHEMA.md; понятные ошибки на нет пакета/версии/
  подписи (сценарные тесты).
- [ ] stdlib.collections — ВЕРИФИКАЦИЯ и докомплектование: deque/set/
  priority_queue уже написаны на Datara (467 строк, вопреки пометке v150
  «каркас — очередь»), но НЕТ тестов в stdlib_tests. Добавить @test-сьют:
  deque (push_front/push_back/pop обе стороны, FIFO/LIFO), set (add/contains/
  дубликаты), priority_queue (heap-порядок, update). Критерий: ноль
  багов-тихо; каждый найденный баг → фикс в этой же фазе.
- [ ] stdlib.json замер vs C (остаток M12): парсер на 10 МБ JSON — Datara vs
  C-эталон; если Datara медленнее ×2 — C-ядро под капотом, Datara-API
  (правило из v150). Артефакт bench_json/RESULTS.md.

## N4. DX: migrate, docs, LSP `[ ]`

- [ ] `forgen migrate 1.4 1.5` — механические правки из migration guide:
  let→val/mut (сохранение семантики mut-мест), String→Str, println→out
  (одноарг), отчёт {файл, строка, правка}. Round-trip тест: миграция
  fixtures/legacy-примеров → компиляция 0 errors; `--dry-run` печатает дифф
  без записи.
- [ ] README/language tour ревизия под 1.5: секции fiber (`is`-пример,
  `inspect --memory`, `forgen profile`), замена цифр тестов; примеры
  проверяются прогоном (каждый сниппет туториала — в CI-смоук-список,
  как examples).
- [ ] LSP под 1.5-фичи: completions для fiber_*/chan-методов, hover для
  `is`-сужения (тип в ветке), диагностика E-ANY-001 в publishDiagnostics.
  Тест в test_lsp_advanced.rs.
- [ ] **ЯЗЫКОВОЙ МАНУАЛ × 1.5-ФИЧИ** (главная док-дыра): Reference Manual
  (17 секций) не знает о релизных фичах — 0 упоминаний `is Int`, fiber,
  E-ANY-001, `inspect --memory`. Добавить секции: §4.x «Dynamic Values &
  TSU» (val/is/narrowing/E-ANY-001/NaN-box), §8.x «Fibers & NatVar»
  (spawn/yield/sleep/join/каналы), §11.x «return-? и Outcome-конструкторы»,
  §16.x «inspect --memory / profile / migrate / dpm». Каждый пример —
  компилируемый и в CI-смоук-списке. Критерий: grep-гейд-тест
  tests/test_docs_coverage.rs — секции существуют и примеры запускаются.
- [ ] `forgen watch` — SLA-гейт: watch уже реализован (cmd_watch), но без
  интеграционного теста и замера. Зафиксировать контракт: сохранение →
  диагностика < 200 мс (инкрементальный кэш уже даёт hit 15–31 мс), вывод
  понятен (файл/строка/код). Тест: временный проект, модификация файла,
  замер; мутационный кейс (поломка → красная диагностика → фикс → зелёная).
- [ ] `forgen fmt` — гейт идемпотентности: fmt(x) == fmt(fmt(x)) на всех
  examples + stdlib; `fmt --check` в CI-джобе quality. Тест: фаззи-прогон на
  examples — ноль диффов второго прохода.
- [ ] Resolver-hint для новичка: «Undefined symbol 'clamp'» при импортированном
  пакете с clamp → help «add `use <pkg>` (found in package 'mathx')».
  Точнее: скан packages/*/package.json entry на неимпортированный символ.
  Тест: consumer без use → подсказка с именем пакета.
- [ ] CONTRIBUTING.md ревизия: собрать чеклист контрибьютора из реального
  гейта (build/clippy/test ×3, 60KB-гейт, смоуки) + «как добавить stdlib-
  модуль» + «как добавить тест-сьют». Лёгкая ревизия существующего.
- [ ] docs/DPM_SCHEMA.md (перенос из N3) + ссылка из README.
- [ ] Дока для первых пользователей: одностраничный «Datara for C/Rust/Python
  developers» — три колонки «как было / как в Datara / почему быстрее или
  безопаснее» (без маркетинговых заявлений, только проверяемые таблицы из
  PLAN_V150 §3.5/bench).

## N5. ABI freeze — подготовка (ранний срез H) `[ ]`

- [ ] docs/abi.md черновик: C-рантайм API (датские префиксы datara_rt_*),
  String ABI, layouts List/Map/Outcome/Maybe, val NaN-box теги, bridge-
  примитивы, datara.toml-схема. Пометка «frozen from 1.5.0».
- [ ] abi-тесты: статический список экспортов runtime (dumpbin/nm →
  golden-файл), layout-ассерты (sizeof/alignof val=8, Outcome=24 — по
  факту), ABI-контрактный тест «старый .obj линкуется с новым runtime»
  (один кейс). Изменение ABI в 1.5.x = ломающее → запрещено после фриза.

## N7. Hub & Showcase: воплощение «языка-хаба» `[ ]`

Идентичность Datara — хаб: лучшее из Python/C/Rust/Fortran и градуируемый
контроль (проще/сложнее по желанию). В 1.5.0 это существовало частями,
но не было собрано в показательные артефакты.

- [ ] **Flagship-демо пайплайна** examples/showcase/pipeline_demo: одна
  программа — чтение CSV (io.fs) → агрегация на Datara (collections + Dec64)
  → SIMD-ядро → HTTP-ответ (net/socket) → вызов Python (interop.python).
  Golden-output тест: одинаковый stdout на трёх бэкендах. Это главный
  маркетинговый артефакт релиза — «хаб в одном файле».
- [ ] **Bridge tour**: по одному компилируемому примеру на каждый мост
  (c/python/js/rust) в bridges/examples/, локальный runner-скрипт
  tools/bridge_smoke.py (не зависящий от CI): прогон всех мостов одной
  командой, таблица статусов. Критерий: скрипт зелёный на dev-машине;
  CI-интеграция — N6 при наличии раннеров.
- [ ] **Levels-верификация** (градуируемый контроль): README заявляет 4
  уровня (Scripting/Enterprise/Systems/Kernel) — построить conformance-сьют
  tests/test_levels.rs: Scripting (out/val/match/пакеты), Enterprise
  (Outcome/capabilities/behavior), Systems (unsafe/FFI/SoA/Layer-1),
  Kernel (asm/inline_hint/pool) — каждая фича уровня реально исполняется в
  тесте. Критерий: 4/4 уровня зелёные = «уровни» перестают быть декларацией.
- [ ] **WASM playground**: wasm-бэкенд уже генерит .wasm/.js/.html
  (01_vertical_slice) — собрать docs/playground/ статическую страницу с 3–5
  готовыми демо (компилируются `forgen build --target wasm`, страница
  открывается без сервера). Критерий: страница работает в браузере; билд-
  скрипт воспроизводит артефакты одной командой.
- [ ] «Graduated control» в манул: интро-таблица «одна задача — три уровня
  написания» (скриптовый val/out → enterprise Outcome/behavior → systems
  unsafe/SoA) на задаче FizzBuzz-класса; связка с levels-верификацией.

## N6. Релиз-инжиниринг `[!]` (требует от человека)

- [!] Push в GitHub + тег v1.5.1: CI (security-audit, m4-stress первый
  прогон) — подтвердить зелёность или лог падения. Нужен push-доступ
  (токен выдан владельцем; использовать только в момент push, ротировать
  после).
- [!] Installers: release.yml сборка Windows/Linux/macOS артефактов +
  smoketest установщика. Нужен тег и релиз-раннеры.
- [!] Онлайн-реестр (GitHub Pages): обновить index.json с подписанными
  пакетами (после N3-подписи и push).
- [ ] Dockerfile ревизия (файл существует, 38 строк): rust-тулчейн версия,
  build гейта, run 01_hello в контейнере; опубликовать образ GHCR при push.
  Критерий: `docker build` + `docker run ... forgen run examples/01` внутри
  контейнера печатают hello.

---

## Порядок работ (зависимости)

N0 → N1 (гигиена и баги — фундамент доверия) → N2/N3 параллельно → N4 →
N5 в фоне → N6 последним (нужен человек). N0 и N1 — обязательный вход в
релиз; N2 — самая крупная фаза (2–3 недели); N3/N4 — по неделе.

## Критерии готовности 1.5.1 (DoD)

1. Полный гейт зелёный ×3 прогона подряд (флейков нет).
2. Реестр багов v150 §5: ноль открытых `[ ]` (все закрыты или перенесены в
   H с пометкой).
3. NatVar: многопоточный режим за флагом, echo-тест зелёный, честный
   tokio-отчёт опубликован (включая проигрыши, если есть).
4. Реестр: 5 пакетов, подпись Ed25519 работает локально, bridge check.
5. `forgen migrate 1.4 1.5` мигрирует legacy-фикстуры в компилируемый код.
6. docs/abi.md + abi-тесты — ABI 1.5.0 зафиксирован.
7. Обратная совместимость: все тесты 1.5.0 проходят без правок семантики.
8. **Языковой мануал**: секции TSU/Fibers/E-ANY/inspect написаны, примеры
   компилируются и в CI-смоуке (сейчас — 0 упоминаний).
9. **Hub-артефакты**: flagship pipeline_demo (3 бэкенда, golden output),
   bridge tour зелёный одной командой, levels-верификация 4/4.
10. ** Playground**: WASM-страница с демо работает в браузере, билд
    воспроизводится одной командой.
11. **DX-гейты**: watch < 200 мс интеграционно, fmt идемпотентен + --check
    в CI, resolver-подсказка «add use <pkg>» для новичка.
12. **collections**: deque/set/priority_queue покрыты тестами, все найденные
    баги закрыты.

Порядок: N0 → N1 → N2 ∥ N3 ∥ N7 → N4 → N5 (фон) → N6 (человек).
N7 — рекомендован к приоритету: это единственная фаза, которая превращает
внутренние достижения 1.5.0 в видимые артефакты для внешнего пользователя.
