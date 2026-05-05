TUI process-tree monitor for a single PID and its descendants.

```
prowl <pid> [--interval 1000] [--threads]
```

Keys: `q`/`Esc` quit, `↑↓` or `j`/`k` navigate, `Enter`/`Space` collapse subtree, `t` toggle threads.

Reads from `/proc`, so Linux-only. The header shows a CPU graph, memory bar, IO rates, and elapsed/CPU time for the root process; the table below lists the subprocess tree with the same metrics per row.

See `current.png` and `narrow.png` for what it looks like.
