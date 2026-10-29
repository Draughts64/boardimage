# boardimage

**[English](README.md)** | Русский

[![CI](https://github.com/Draughts64/boardimage/actions/workflows/ci.yml/badge.svg)](https://github.com/Draughts64/boardimage/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Draughts64/boardimage)](https://github.com/Draughts64/boardimage/releases)
[![License](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)

Сервис рендера игровых досок: **шашки** (18 вариантов по PDN —
международные, русские, итальянские, канадские, турецкие…) и
**шахматы**. Картинки позиций **PNG / WebP / SVG** и **GIF-реплеи**
партий. Rust, ноль состояния, двухуровневый кэш, метрики Prometheus,
OpenAPI/Swagger. Работает и как web-сервис, и как CLI.

Полное техническое задание: **[SPEC.md](SPEC.md)**.

## Демо

Всё генерируется CLI из каталога [`demo/`](demo/) —
**[онлайн-галерея](https://draughts64.github.io/boardimage/)**,
шахматная витрина отдельно:
**[/chess.html](https://draughts64.github.io/boardimage/chess.html)**.

| | | |
|---|---|---|
| ![Шахматы: стрелки + кириллица](demo/46-chess-arrows.png) | ![Анализ-борд шашек](demo/33-analysis-engine.png) | ![Русские: номера+рамка](demo/34-russian-full-combo.png) |
| **шахматы**: стрелки UCI, lastMove, подписи | **шашки**: движковые стрелки + heatmap | русские: номера 1..32 + рамка + тема |
| ![Итальянские: светлые клетки](demo/38-italian-caption.png) | ![Кастомная тема neon](demo/35-neon-numeric-border.png) | ![Турецкие: все клетки](demo/36-turkish-wood-arrows.png) |
| итальянские: светлые клетки + кириллица | кастомная тема (SVG-спрайты) | турецкие: все 64 клетки |
| ![Канадские 12×12](demo/37-canadian-arrows.png) | ![Переворот чёрными](demo/39-flipped-arrows.png) | ![Форматы](demo/09-formats.png) |
| канадские 12×12 + стрелки | ориентация чёрными + рамка | PNG 16.8К / WebP 7.3К / SVG 5К |

Анимация — GIF-реплеи с полосой подписей (кириллица встроенным шрифтом
DejaVu Sans, комментарий пишется в метаданные файла):

<p>
<img src="demo/52-chess-fools-mate.gif" width="280" alt="Шахматы: дурачий мат">
<img src="demo/47-chess-opera.gif" width="280" alt="Шахматы: Оперная партия">
<img src="demo/44-italian-replay.gif" width="280" alt="Шашки: итальянская партия">
</p>

## Шахматы

`--game chess` (сервер: `?game=chess`) переключает сервис в шахматный
режим:

- обычный **шахматный FEN** по [PGN Standard §16.1.3](http://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm)
  (`rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1`);
  счётчики полуходов можно опускать (EPD), рокировки Shredder-FEN
  (`AHah`) принимаются, en-passant валидируется спекой;
- встроенный набор фигур **cburnett** (растеризация под клетку,
  кэширование, атрибуция лицензии в `assets/pieces/cburnett/`);
- стрелки и lastMove в UCI (`Ge2e4`, `e2e4`), heatmap-подсветка, темы
  доски, рамка a..h/1..8, координаты — всё из шашечного режима;
- GIF-реплеи: `--game chess` + повторяемый `--fen` на каждую позицию.

```bash
boardimage render --game chess --arrows "Ge2e4,Bg1f3" --last-move e2e4 \
  --white "Карлсен" --black "Каруана" --comment "WCh 2018" \
  --fen "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1" \
  --out m.png
```

## Быстро

```bash
cargo run                    # web: http://127.0.0.1:8080/healthz
cargo run -- render ...      # CLI без сервера
docker build -t boardimage . # прод-образ (~15 МБ)
docker run -p 8080:8080 ghcr.io/draughts64/boardimage   # готовый из GHCR
```

## Docker

Готовые образы публикуются в GHCR при каждом пуше: `latest` (master) и
версии по тегам (`v0.1.0`, …).

```bash
docker pull ghcr.io/draughts64/boardimage:latest
docker run -d --name boardimage -p 8080:8080 \
  -e CACHE_MB=256 \
  -v boardimage_cache:/cache -e CACHE_DIR=/cache -e CACHE_MB_DISK=2048 \
  -v ./my-themes:/themes:ro \
  ghcr.io/draughts64/boardimage:latest
curl http://127.0.0.1:8080/healthz   # ok
```

В образ уже вшиты примеры тем (`/themes/neon`); свои темы монтируются
поверх. Все переменные: `BIND` (0.0.0.0:8080), `CACHE_MB` (LRU в памяти),
`CACHE_DIR` + `CACHE_MB_DISK` (дисковый кэш с вытеснением), `THEMES_DIR`
(каталог тем, по умолчанию `/themes`).

> Пока пакет GHCR приватный, `docker pull` требует
> `docker login ghcr.io` (PAT со скоупом `read:packages`). После
> переключения видимости пакета в public — работает без логина.

## CLI: `boardimage render`

Формат выбирается расширением `--out`: `.png` / `.svg` / `.webp` / `.gif`
(по умолчанию PNG). Неизвестный флаг — ошибка со списком разрешённых.

```bash
# шашки, старт 10×10: числовые клетки, диапазоны, движковые :H0:F1 ок
boardimage render --fen "W:W31-50:B1-20:H0:F1" --out start.png --size 512

# шашки, алгебраические клетки (a1..k10), доска 8×8 english
boardimage render --game-type 21 --fen "W:Wa3,g3,b2:Bb8,Kc3" --out pos.png

# шашки, полный PDN GameType-тег; нотация по умолчанию возьмётся из тега
boardimage render --game-type "21,B,8,8,N1,0" --fen "B:W18,K10:B12,K22" --out e.svg

# шашки: номера на клетках + алгебраическая рамка + кириллица
boardimage render --variant russian --notation numeric --margin 24 \
  --white "Сидорова А." --black "Кузнецов Д." --last-move 2521 \
  --fen "W:W21-32:B1-12" --out r.png

# шашки, анализ-борд: стрелки движка + heatmap + lastMove
boardimage render --arrows "4741,3228" --squares "23:0.7" --last-move 4741 \
  --comment "Движок: 47-41 (+1.2)" --fen "W:W36,K47:B5,23" --out a.png

# GIF-реплей (шашки или шахматы): --fen повторяется по кадрам
boardimage render --variant russian --delay-ms 800 \
  --white "Сидорова Анна" --black "Кузнецов Д." --comment "Блиц" \
  --fen "W:W21-32:B1-12" --fen "W:W21-30,25,32:B1-12" \
  --fen "W:W21-30,25,32:B1-10,12,18" --out game.gif
```

| Флаг | Значение | По умолчанию |
|---|---|---|
| `--game chess` | шахматный режим (FEN + фигуры cburnett) | шашки |
| `--fen <FEN>` | позиция; повторяйте для GIF-кадров | обязателен |
| `--out <file>` | `.png` / `.svg` / `.webp` / `.gif` по расширению | обязателен |
| `--size <16..1024>` | сторона картинки (GIF ≤640) | 512 |
| `--game-type <t>` | число PDN (20–41, 96–98) или полный тег `21,B,8,8,N1,0` | 20 |
| `--variant <name>` | standard, russian, italian, turkish, … | standard |
| `--board-size` / `--board-height` | своя доска (чётные 2..16) | 10×10 |
| `--numbering <0..3>` | начало клетки 1: 0 низ-слева … 2 верх-слева (канон) | по варианту |
| `--light-squares` | рабочие клетки светлые | по варианту |
| `--theme <name>` | встроенная или из `THEMES_DIR` | lidraughts-brown |
| `--light` / `--dark` | инлайн-цвета доски `#rrggbb` | по теме |
| `--notation` | none / numeric / algebraic / coords | none |
| `--margin <px>` | поля вокруг доски | 0 |
| `--border` / `--no-border` | алгебраическая рамка (сочетается с numeric) | вкл при margin>0 |
| `--last-move <m>` | `3228`, `32-28`, `c3-b4`, `e2e4` — подсветка хода | — |
| `--arrows <csv>` | `G3228,R0622,44` / `Ge2e4` — стрелки + круг | — |
| `--squares <csv>` | `46:0.9,12` — подсветка клеток с интенсивностью | — |
| `--white` / `--black` | имена (Unicode/кириллица) — полоса подписи | — |
| `--comment <текст>` | подпись; в GIF пишется в метаданные файла | — |
| `--delay-ms <100..3000>` | задержка GIF-кадров | 700 |
| `--quality <1..100>` | WebP; 100 = lossless | 85 |
| `--flip` / `--flat` / `--transparent` | ориентация чёрными / плоский стиль / прозрачный фон | — |

## Web API (кратко — полностью в SPEC.md и /swagger)

```
# шашки
GET /board.png?fen=W:W31-50:B1-20&boardSize=10&size=1024
GET /board.png?fen=W:Wa1,c1:Bb8&gameType=21        # алгебраический FEN
GET /board.png?fen=...&gameType=22                 # итальянские (светлые клетки)
GET /board.png?fen=...&notation=numeric            # номера 1..50 у каждой клетки
# шахматы
GET /board.png?game=chess&fen=rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR%20w%20KQkq%20-%200%201
GET /board.webp?game=chess&fen=...&quality=85      # 100 = lossless
GET /board.svg?fen=...&arrows=G0622,44             # стрелки как на analysis
POST /game.gif   { "game": "chess", "frames": [ {"fen": "...", "lastMove": "e2e4"} ] }
```

`GET /openapi.json` — машина; `/swagger` и `/redoc` — документация;
`/metrics` — Prometheus; `/healthz` — живость.

Движка правил в сервисе нет: партию проигрывает клиент (монолит
[Draughts64/draughts](https://github.com/Draughts64/draughts) для шашек
— архитектура lila-gif) и присылает готовые кадры.

## FEN

**Шашки** — полный PDN 3.0 ([wiegerw.github.io/pdn/fen.html](https://wiegerw.github.io/pdn/fen.html)):
ход `W`/`B`/`?`, списки обеих сторон (можно пустые), префиксы `K`
(король), `G`/`P` (призрачные; префиксы — только ЗАГЛАВНЫЕ, строчные
k/g — это файлы алгебраических имён), диапазоны `31-50`, движковые
суффиксы `:H0:F1` пропускаются. Клетки задаются числами (канон
движка/FMJD: клетка 1 сверху-слева, белые снизу) **или алгебраически**
(`a1`, `Kc3`); на досках шириной 10 буква `j` пропускается (`a…i,k`).

**Шахматы** — стандартный FEN (см. секцию «Шахматы» выше): шесть полей,
счётчики опциональны, en-passant только 3/6 ранг.

Вариант шашечьей доски задаётся `gameType` (число 20–41, 96–98 — или
полный тег `"21,B,8,8,N1,0"`), `variant`-алиасом или парой
`boardSize`/`boardHeight` (+ `lightSquares`, `allSquares`, `numbering`).

## Кастомные темы (как у шахматистов)

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
встраивается как data-URI. В шахматном режиме темы красят доску, а
фигуры cburnett остаются (свои шахматные наборы — задел на будущее).
Пример целиком: [`themes/neon/`](themes/neon).

## Статус и ограничения v1

Шашки: 18 вариантов по PDN GameType (итальянские светлые клетки,
канадские 12×12, турецкие все клетки, спанциретти 10×8), нумерация-канон
движка lidraughts. Шахматы: полный FEN + cburnett. Общее: стрелки/
кольца/кисти как на analysis (порт draughtsground), Unicode-подписи,
двухуровневый кэш + ETag, 53 автотеста. Задокументированные упрощения:
PNG полноцветный RGBA (palette-квантование не делается), формальный
нагрузочный тест не прогонялся.

## Лицензия

AGPL-3.0. API-контракт — из
[niklasf/web-boardimage](https://github.com/niklasf/web-boardimage),
приёмы GIF — из [lichess-org/lila-gif](https://github.com/lichess-org/lila-gif),
шрифт подписей — DejaVu Sans (лицензия в `assets/`),
шахматные фигуры — cburnett, GPL-2.0-or-later (`assets/pieces/cburnett/`).
