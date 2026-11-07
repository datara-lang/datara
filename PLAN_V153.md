# PLAN_V153 — «Production-Ready»: язык для реальной работы

Цель: из «инженерный артефакт 8/10» → «язык, на котором пишут рабочие
программы». Три кита: **библиотеки** (CLI/time/fs/net), **стабильность**
(стресс + ASan + флейки), **скорость и безопасность** (честные бенчи vs
Rust + документированная safety-история).

---

## 0. Сводка текущего состояния (все факты верифицированы, 2026-11-07)

### Чем язык уже знаменит (cool facts — использовать в README/доках)

| Факт | Доказательство |
|---|---|
| **3 бекенда байт-в-байт**: Cranelift AOT, JIT-host, LLVM (+WASM) | tools/parity_matrix.py, test_pipeline_demo golden |
| **Безопасность через доказательства, не рантайм**: TMM dataflow-lifetime — владение 100% proven, 0% guarded | optimization report каждой сборки |
| **Capability-модель**: fs-read/fs-write/net-access только через `unsafe(justification)` | E0940 на каждый непокрытый IO |
| **Proof-Carrying Code**: деление на ноль ловится на компиляции (E0941), require-контракты | security/verify_expr.rs |
| **Фиберы быстрее tokio на ping-понге**: 27.2k vs 25.6k msg/s | bench_natvar/RESULTS.md |
| **Мьютексы с таймаутами**: AB-BA саморазрешается < 1 c | test_fiber_mutex 3/3 |
| **Бенч поймал реальный баг**: str_substring O(n²) → memchr → **663×** (73.5s → 111ms на 10 МБ) | bench_json/RESULTS.md |
| **JSON-валидатор побитово совпадает с C**: 124 259 записей / 10 МБ за 152 ms | bench_json/RESULTS.md |
| **60 KB гейт на файл исходников** + evidence-gate: Applied без IR-дельты = Reject | test_release_file_size_limits, evidence.rs |
| **dpm**: Merkle+SHA256+Ed25519 подпись пакетов, trust-store эскалация | test_dpm_signing |
| **Scheduler fairness**: yield-поллер не голодает спящих | test_sched_fairness |
| **CI полностью зелёный** на main, ~1550 тестов / 320+ сьютов | GitHub Actions 8aeb427 |

### Для каких задач язык УЖЕ подходит (сегодня, с доказательствами)

1. **Скрипты и утилиты**: fs/csv/json/text/env/args + мгновенный запуск
   (JIT-host), миграция с Python-подобного синтаксиса (`forgen migrate`).
2. **Сетевые сервисы на фиберах**: fiber_net_recv (IO-парковка),
   мьютексы с таймаутами, minihttp demo, stackful-стеки (полные
   фреймы вызовов, не ленивые future).
3. **Производительные ядра**: SIMD-интринзики, parallel_for,
   LLVM -O3, `-march=x86-64-v2` бейзлайн; рантайм-функции на чистом C.
4. **Data-пайплайны**: bench_json 10 МБ за 152 ms (валидация) +
   124 ms (extract) на Cranelift — конкурентно C -O2 с зазором ≤ 11×.
5. **Обучение и системные эксперименты**: 3 читаемых бекенда,
   evidence-трейс каждого решения оптимизатора, честные бенчи.

### Для каких задач НЕ подходит сейчас → план закрытия

| Задача | Что мешает | Пункт плана |
|---|---|---|
| HTTP-клиент / TCP-утилиты | Нет stdlib.net-обёртки | **A2** |
| CLI-инструменты с флагами | Нет stdlib.cli (args есть в рантайме) | **A1** |
| Логи с датами, cron-подобное | Нет stdlib.time (now_ms есть) | **A3** |
| Работа с деревом файлов | Нет mkdir/remove/rename/size/mtime | **A4** |
| «Библиотека сломалась в проде» | Нет стресс/ASan-прогонов в CI | **B1** |
| Заявление «быстрее Rust» | Бенч-матрица не обновлялась | **C1** |
| Отладка брейкпоинтами | DWARF есть, VS Code-интеграции нет | **D2** |
| Безопасная работа с мутабельным глобальным состоянием | #10b: mut-глобалы изолированы per-function | **E2** |

---

## A. Библиотеки «первого дня» (пишутся НА Datara поверх готового рантайма)

### A1. stdlib.cli — аргументы и окружение `[x]` ЗАКРЫТО 2026-11-07
- [x] `args() -> List<Str>`, `env_get/env_default`
- [x] `parse_args -> Args` (values/flags/rest), `arg_get/arg_flag`
- [x] Верифицировано на Cranelift+JIT (CL/LLVM — через parity-сьют)
- По пути ПОЧИНЕНЫ 2 компиляторных бага (repro #12):
  1. GetField не пропагировал класс payload'а Result в val_to_class →
     `parsed.value.get(...)` диспетчился в чужой 1-арг builtin →
     Cranelift ABI assert (arity 2 vs 1);
  2. DCE резолвил методы с именами интринсиков ("get") как интринсики,
     НЕ держа одноимённые юзер-методы живыми → калли выживали,
     калли-цели удалялись до декларации → matched=None.
  Регресс: tests/test_result_field_method.rs 2/2.
  Грамматика: `--name value`, булев-флаг — только если следом флаг или
  конец аргументов (документировано в модуле).

### A2. stdlib.net — TCP поверх fiber-сокетов `[ ]`
- [ ] `TcpListener.bind(port) -> Result<Listener>`, `.accept() -> Result<TcpStream>`
- [ ] `TcpStream.connect(host, port) -> Result<TcpStream>`,
  `.send(Str)`, `.recv(max, timeout) -> Result<Str>`, `.close()`
- [ ] Echo-сервер на фиберах: 100 conn / 1000 msg (N1-критерий)
- [ ] HTTP/0.9-демо поверх потока (minihttp уже есть — обернуть)

### A3. stdlib.time — ЗАКРЫТО 2026-11-07
- [x] `now_unix_ms` (проводка добавлена), `now_ms/now_precise_ms/now_ns`
- [x] `date_parts(unix_ms) -> List<Int>` [year,month,day,h,m,s, localtime] —
  новый C-хелпер (datara_fs.c, localtime_s/localtime)
- [x] `fmt_date` ("YYYY-MM-DD hh:mm:ss") + `fmt_date_iso` (T-разделитель)
- [x] `seconds_between`. Верифицировано CL+LLVM (настоящая дата).
  Урок: time.h нужен и в WIN32-ветке datara_fs.c

### A4. stdlib.fs — ЗАКРЫТО 2026-11-07
- [x] Новые C-хелперы: fs_mkdir (mkdir -p!), fs_remove, fs_rename
  (MoveFileExA BOOL напрямую — GetLastError-стайл давал ложный -1),
  fs_file_size, fs_mtime — capability-gated (fs-write/fs-read)
- [x] stdlib/fs/fskit.dtr: walk (рекурсивный), dir_size, file_count,
  newest_mtime — НА Datara
- [x] Верифицировано CL+LLVM: mkdir -p / size=9 / walk / rename /
  remove / mtime — все зелёные

## B. Стабильность `[ ]`
- [ ] **B1**: стресс-сьют — 100 программ × мусорный ввод (fuzz-lite),
  ни одного висяка/креша; ASan-прогон рантайма в CI-джобе
- [ ] **B2**: soak — bench_natvar echo 10 мин без роста RSS
- [ ] **B3**: детерминизм — одинаковый вывод программы на 3 тирах для
  ВСЕХ examples (расширение parity_matrix)

## C. Скорость — честная матрица vs Rust `[ ]`
- [ ] **C1**: обновить bench_datara_vs_rust: arith/loop/str/json/http-echo;
  публиковать ЧТО быстрее и НАСКОЛЬКО (правило честности)
- [ ] **C2**: hotspot-профилирование ( sampler / ETW) топ-3 бенчей
- [ ] **C3**: llvm-gate: ни одного silent-fallback в тестах

## D. Удобство `[ ]`
- [ ] **D1**: `forgen new --template cli|server|script` (шаблоны проектов)
- [ ] **D2**: VS Code debug-конфигурация (cppvsdb → cdb attach к
  debug-бинарю) + документ «Отладка за 5 минут»
- [ ] **D3**: error-messages ревизия: у E-кодОВ — готовые «как починить»
  примеры в explain

## E. Ядро — накопленные репро (1.5.3)
- [ ] **E1**: #11 — bare `m[k]` is-нарровинг: тегированные слоты мапы
  (миграция json-парсера)
- [ ] **E2**: #10b — mut-глобалы между функциями: общий data-slot путь
  (+ атомики для MT)
- [ ] **E3**: IOCP/epoll IO-поллер (select() → 10k+ соединений)
- [ ] **E4**: async sugar поверх FiberHandle

## Порядок работы
A1 → A3 → A4 (быстрые, чистый Datara) → A2 (net) → C1 (честный бенч) →
B1 (стресс) → D1 → E1-E4 → релиз 1.5.3.

**Критерий готовности (DoD)**: «напиши HTTP-логгер с флагами CLI и
датой в логе, не покидая stdlib» — работает из коробки, parity на 3
тирах, CI зелёный, бенчи против Rust опубликованы честно.
