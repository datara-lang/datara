# Evidence — верификационные артефакты PLAN_V150/V151

Каждый файл здесь — доказательство из чекбокса плана (команда + вывод).

- `m4/` — стресс-отчёты large-program (10k–200k), A/B Arc/noArc, fastpath
- `m5_deref/` — @arc vs raw deref-бенчмарк (PLAN_V150 M5 замер)
- `m6/` — probe-файлы dataflow-слоя (fresh_return, leak_diff, cyclic)
- `natvar/` — NatVar fiber-бенчмарки (срез 1)
- `probes/` — минимальные .dtr-репродукторы багов и фиксов (по каталогам фаз)

Бинарники (exe/obj/pdb) в evidence не попадают — только исходники и отчёты.
