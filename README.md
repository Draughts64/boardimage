# boardimage

Сервис рендера шашечных досок: **PNG / WebP / SVG** картинки позиций и
**GIF-реплеи** партий. Rust, ноль состояния, двухуровневый кэш, метрики
Prometheus, OpenAPI/Swagger. Работает и как web-сервис, и как CLI.

Полное техническое задание: **[SPEC.md](SPEC.md)** (API, все варианты
шашек по PDN, нотация у каждой клетки, темы, кэш).

## Быстро

```bash
cargo run                    # web: http://127.0.0.1:8080/healthz
cargo run -- render ...      # CLI без сервера
docker build -t boardimage . # прод-образ (~15 МБ)
```

Окружение web-сервера: `BIND` (по умолчанию 0.0.0.0:8080), `CACHE_MB`
(in-memory LRU), `CACHE_DIR` + `CACHE_MB_DISK` (дисковый кэш с вытеснением
по размеру), `THEMES_DIR` (каталог кастомных тем, см. ниже).

## CLI: `boardimage render`

Рендер в файл напрямую, без сервера. Формат выбирается расширением
`--out`: `.png` (по умолчанию), `.svg`, `.gif`. Примеры:

```bash
# классический старт 10×10 (числовые клетки, диапазоны, движковые :H0:F1 ок)
boardimage render --fen "W:W31-50:B1-20:H0:F1" --out start.png --size 512

# алгебраические клетки (a1…k10) — FEN из любого генератора; доска 8×8 english
boardimage render --game-type 21 --fen \
  "W:Wa3,g3,b2,d2,f2,h2,a1,c1,e1,g1:Bb8,d8,Kc3" --out pos.png

# полный PDN GameType-тег вместо числа; нотация по умолчанию возьмётся из тега
boardimage render --game-type "21,B,8,8,N1,0" --fen "B:W18,K10:B12,K22" --out e.svg

# русские шашки, номера 1..32 на клетках, тема
boardimage render --variant russian --notation numeric --theme green \
  --fen "W:W21-32:B1-12" --out r.png

# GIF-реплей: --fen повторяется, кадры в порядке ходов
boardimage render --variant standard --delay-ms 800 \
  --fen "W:W31-50:B1-20" --fen "W:W31-30:B1-20" \
  --white "Ivanov" --black "Petrov" --comment "Tournament 2026" --out game.gif
```

Все флаги:

| Флаг | Значение | По умолчанию |
|---|---|---|
| `--fen <FEN>` | позиция; повторяйте для GIF-кадров | обязателен |
| `--out <file>` | `.png` / `.svg` / `.gif` по расширению | обязателен |
| `--size <16..1024>` | сторона картинки (GIF: ≤640) | 512 |
| `--game-type <t>` | число PDN (20, 22, 25, …) или полный тег `21,B,8,8,N1,0` | 20 |
| `--variant <name>` | standard, russian, italian, turkish, … | standard |
| `--board-size`/`--board-height` | своя доска (чётные 2..16) | 10×10 |
| `--numbering <0..3>` | начало клетки 1: 0 низ-слева … 2 верх-слева (канон) | по варианту |
| `--light-squares` | рабочие клетки светлые | по варианту |
| `--theme <name>` | встроенная или из `THEMES_DIR` | lidraughts-brown |
| `--light` / `--dark` | инлайн-цвета доски `#rrggbb` | по теме |
| `--notation` | none / numeric / algebraic / coords | none |
| `--last-move <m>` | `3228`, `32-28`, `c3-b4` — подсветка последнего хода | — |
| `--white` / `--black` | имена — включают полосу подписи | — |
| `--comment <текст>` | подпись; в GIF пишется в метаданные файла | — |
| `--delay-ms <100..3000>` | задержка GIF-кадров | 700 |
| `--flip` | ориентация чёрными | — |
| `--flat` | плоский стиль шашек | — |
| `--transparent` | прозрачный фон (PNG/WebP) | — |

## Кастомные темы (как у шахмат)

Два способа задать свой дизайн:

```bash
# 1) инлайн — только цвета доски
boardimage render --light "#f0d9b5" --dark "#b58863" --fen ... --out b.png
curl "http://127.0.0.1:9091/board.png?fen=...&light=%23f0d9b5&dark=%23b58863"

# 2) каталог тем THEMES_DIR — цвета + спрайты шашек (SVG или PNG):
#    themes/neon/theme.json:
#    {"light":"#2b2b3b","dark":"#3d3d54",
#     "pieces":{"wm":"wm.svg","bm":"bm.svg","wk":"wk.svg","bk":"bk.svg"}}
THEMES_DIR=./themes boardimage render --theme neon --fen ... --out n.png
```

Спрайты: `wm`/`bm` обязательны, `wk`/`bk` опциональны (упав — простая
шашка + векторная корона); SVG растеризуется под размер клетки и
кэшируется; призрачные шашки — opacity 0.3; в SVG-выводе спрайт
встраивается как data-URI. Пример целиком: [`themes/neon/`](themes/neon).

## Web API (кратко — полностью в SPEC.md и /swagger)

```
GET /board.png?fen=W:W31-50:B1-20&boardSize=10&size=1024
GET /board.webp?fen=...&quality=85            # 100 = lossless
GET /board.svg?fen=...&arrows=G0622,44
GET /board.png?fen=W:Wa1,c1:Bb8&gameType=21   # алгебраический FEN
GET /board.png?fen=...&gameType=22            # итальянские (светлые клетки)
GET /board.png?fen=...&notation=numeric       # номера 1..50 у каждой клетки
POST /game.gif   { "frames": [ {"fen": "...", "lastMove": "4741"} ] }
```

`GET /openapi.json` — машина; `/swagger` и `/redoc` — документация;
`/metrics` — Prometheus; `/healthz` — живость.

Шашечного движка в сервисе нет: партию проигрывает монолит
[Draughts64/draughts](https://github.com/Draughts64/draughts) и
присылает готовые кадры.

## FEN

Принимается полный PDN 3.0 ([wiegerw.github.io/pdn/fen.html](https://wiegerw.github.io/pdn/fen.html)):
ход `W`/`B`/`?`, списки клеток обеих сторон (можно пустые), префиксы
`K` (король), `G`/`P` (призрачные lidraughts), диапазоны `31-50`,
движковые суффиксы `:H0:F1` пропускаются. Клетки задаются числами
(канон движка/FMJD: клетка 1 сверху-слева) **или алгебраически** (`a1`,
`Kc3`) — имена физические, ранг 1 снизу; на досках шириной 10 буква
`j` пропускается (`a…i,k`, `j` принимается как псевдоним `k`).

Вариант доски задаётся `gameType` (число 20–41, 96–98 — или полный тег
`"21,B,8,8,N1,0"`), `variant`-алиасом или парой `boardSize`/`boardHeight`
(+ `lightSquares`, `allSquares`, `numbering`).

## Статус и ограничения v1

Полный рендер 18 вариантов по PDN GameType (включая итальянские светлые
клетки, канадские 12×12, турецкие все клетки, спанциретти 10×8). 28
автотестов (юнит + golden). Задокументованные упрощения: PNG
полноцветный RGBA (palette-квантование не делается), формальный
нагрузочный тест (100 rps p99) не прогонялся.

## Лицензия

AGPL-3.0. API-контракт — из
[niklasf/web-boardimage](https://github.com/niklasf/web-boardimage),
приёмы GIF — из [lichess-org/lila-gif](https://github.com/lichess-org/lila-gif)
(AGPL).
