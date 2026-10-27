# boardimage

Сервис рендера шашечных досок: **PNG / WebP / SVG** картинки позиций и
**GIF-реплеи** партий. Rust, ноль состояния, кэш результатов.

Полное техническое задание: **[SPEC.md](SPEC.md)** (API, все варианты
шашек по PDN, нотация у каждой клетки, темы, спрайт-архитектура).

## Быстро

```bash
cargo run                    # http://127.0.0.1:8080/healthz (stage 0)
docker build -t boardimage . # прод-образ (~15 МБ)
```

## API (кратко — полночно в SPEC.md)

```
GET /board.png?fen=W:W31-50:B1-20&boardSize=10&size=1024
GET /board.webp?fen=...&quality=85
GET /board.svg?fen=...&arrows=G0622,44
GET /board.png?fen=W:W21-32:B1-12&gameType=22          # итальянские (светлые клетки)
GET /board.png?fen=...&notation=numeric                # номера 1..50 у каждой клетки
POST /game.gif   { "frames": [ {"fen": "...", "lastMove": "4741"} ] }
```

Шашечного движка в сервисе нет: партию проигрывает монолит
[Draughts64/draughts](https://github.com/Draughts64/draughts) и
присылает готовые кадры.

## Статус

Stage 0 (скелет: /healthz, CI, Docker). Реализация — по плану SPEC.md §10.

## Лицензия

AGPL-3.0. SVG-фигуры шашек заимствованы из
[RoepStoep/lidraughts-boardimage](https://github.com/RoepStoep/lidraughts-boardimage)
(AGPL, адаптация python-chess), API-контракт — из
[niklasf/web-boardimage](https://github.com/niklasf/web-boardimage),
приёмы GIF — из [lichess-org/lila-gif](https://github.com/lichess-org/lila-gif)
(AGPL).
