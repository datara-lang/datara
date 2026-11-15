# Отладка за 5 минут (PLAN_V153 PD2/PD3)

## 1. Соберите debug-бинарь

```bash
forgen build --debug main.dtr
```

## 2. Отладка в VS Code

Установите расширение **C/C++** (ms-vscode.cpptools). Конфигурации
уже в `.vscode/launch.json`:

- **Debug Datara AOT binary (cdb)** — запускает бинарь под Windows
  debugger (cppvsdb) с брейкпоинтами по исходникам Datara (DWARF
  line tables маппятся на строки .dtr).
- **Debug Datara AOT (attach)** — подключается к уже запущенному
  процессу.

Брейкпоинт: откройте .dtr файл, F9 на строке → debugger остановится
(двоичный код содержит line table).

## 3. Отладка фиберов

Фиберы — настоящие OS fibers (Windows `CreateFiberEx` / POSIX
`ucontext`), каждый виден в debugger как отдельный стек:

```
Thread 1: main (scheduler)
  SwitchToFiber → fiber_1 (your code)
  SwitchToFiber → fiber_2 (network handler, parked on recv)
```

Фибер, заблокированный на `fiber_net_recv`, виден как select() в
scheduler's IO slice. `fiber_sleep_ms` паркуется на timer wheel.

## 4. Диагностика

- `CONTRACT_VIOLATION: <msg>` — require не прошёл (проверьте входные
  данные или ослабьте контракт)
- `CONTRACT_VIOLATION` панfills в release — `require` элиминируется
- E-коды: `forgen explain E0941` — подробное объяснение + как починить

## 5. Отладочные переменные окружения

| Переменная | Что показывает |
|---|---|
| `DATARA_CODEGEN_TRACE=1` | dispatch/protocol/method_call — как кодген
  разрешает вызовы |
| `DATARA_SCHED_TRACE=1` | состояние планировщика фиберов (итерации,
  готовые, done) |
| `DATARA_IO_TRACE=1` | io_wait_poll — парковки на select() |
| `DATARA_BRIDGE_TRACE=1` | Python/JS bridge transfer details |
