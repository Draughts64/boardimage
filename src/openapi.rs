// OpenAPI 3 spec + Swagger UI page (in the spirit of the lichess /api
// explorer): /openapi.json serves the machine spec, /swagger serves the
// UI (swagger-ui-dist from CDN), /redoc serves Redoc as a lighter alt.

pub fn json() -> String {
    r##"{
  "openapi": "3.0.3",
  "info": {
    "title": "boardimage",
    "description": "Шашечные доски: PNG/WebP/SVG картинки позиций и GIF-реплеи. 18 вариантов по PDN GameType, нумерация клеток, темы. Движка правил нет: для GIF партию проигрывает клиент и присылает кадры.",
    "version": "0.1.0",
    "license": { "name": "AGPL-3.0", "url": "https://www.gnu.org/licenses/agpl-3.0.html" }
  },
  "servers": [{ "url": "/" }],
  "paths": {
    "/board.png": { "get": { "summary": "Доска в PNG", "parameters": [{"$ref": "#/components/parameters/Fen"}, {"$ref": "#/components/parameters/BoardSize"}, {"$ref": "#/components/parameters/BoardHeight"}, {"$ref": "#/components/parameters/Size"}, {"$ref": "#/components/parameters/LastMove"}, {"$ref": "#/components/parameters/Arrows"}, {"$ref": "#/components/parameters/Squares"}, {"$ref": "#/components/parameters/Orientation"}, {"$ref": "#/components/parameters/LightSquares"}, {"$ref": "#/components/parameters/GameType"}, {"$ref": "#/components/parameters/Variant"}, {"$ref": "#/components/parameters/Notation"}, {"$ref": "#/components/parameters/Numbering"}, {"$ref": "#/components/parameters/PieceStyle"}, {"$ref": "#/components/parameters/Theme"}, {"$ref": "#/components/parameters/White"}, {"$ref": "#/components/parameters/Black"}, {"$ref": "#/components/parameters/Comment"}, {"$ref": "#/components/parameters/Margin"}, {"$ref": "#/components/parameters/Transparent"}], "responses": { "200": { "description": "PNG", "content": { "image/png": { "schema": { "type": "string", "format": "binary" } } } }, "400": { "$ref": "#/components/responses/BadRequest" } } } },
    "/board.webp": { "get": { "summary": "Доска в WebP", "parameters": [{"$ref": "#/components/parameters/Fen"}, {"$ref": "#/components/parameters/Quality"}], "responses": { "200": { "description": "WebP", "content": { "image/webp": {} } }, "400": { "$ref": "#/components/responses/BadRequest" } } } },
    "/board.svg": { "get": { "summary": "Доска в SVG", "parameters": [{"$ref": "#/components/parameters/Fen"}], "responses": { "200": { "description": "SVG", "content": { "image/svg+xml": {} } }, "400": { "$ref": "#/components/responses/BadRequest" } } } },
    "/game.gif": { "post": { "summary": "GIF-реплей (кадры проигрывает клиент)", "requestBody": { "required": true, "content": { "application/json": { "schema": { "$ref": "#/components/schemas/GifRequest" } } } }, "responses": { "200": { "description": "GIF", "content": { "image/gif": {} } }, "400": { "$ref": "#/components/responses/BadRequest" } } } },
    "/healthz": { "get": { "summary": "Живость", "responses": { "200": { "description": "ok" } } } },
    "/metrics": { "get": { "summary": "Prometheus-метрики", "responses": { "200": { "description": "text/plain" } } } }
  },
  "components": {
    "responses": { "BadRequest": { "description": "Невалидный параметр (invalid fen / invalid boardsize / fen required / …)" } },
    "parameters": {
      "Fen": { "name": "fen", "in": "query", "required": true, "schema": { "type": "string" }, "example": "W:Wa3,g3,b2:Bb8,Kc3", "description": "Шашечный FEN PDN 3.0: turn(W|B|?):W<клетки>:B<клетки>. Клетки числовые (31, диапазоны 31-50) ИЛИ алгебраические (a1, Kc3), можно вперемешку. K=король, G/P=призрачные, суффиксы :H0:F1 движка пропускаются. Пример: W:W31-50:B1-20" },
      "BoardSize": { "name": "boardSize", "in": "query", "schema": { "type": "integer", "default": 10, "enum": [8, 10, 12] }, "description": "Ширина доски (чётная 2..16); высота по умолчанию равна ширине" },
      "BoardHeight": { "name": "boardHeight", "in": "query", "schema": { "type": "integer" }, "description": "Высота для неквадратных досок (Спанциретти 10×8)" },
      "Size": { "name": "size", "in": "query", "schema": { "type": "integer", "default": 360, "minimum": 16, "maximum": 1024 } },
      "Quality": { "name": "quality", "in": "query", "schema": { "type": "integer", "default": 85, "minimum": 1, "maximum": 100 }, "description": "WebP; 100 = lossless" },
      "LastMove": { "name": "lastMove", "in": "query", "schema": { "type": "string", "example": "4741" }, "description": "UCI-номера клеток from+to — подсветка" },
      "Arrows": { "name": "arrows", "in": "query", "schema": { "type": "string", "example": "G0622,44" }, "description": "CSV [GBRY]?<from><to>; from==to — круг" },
      "Squares": { "name": "squares", "in": "query", "schema": { "type": "string", "example": "12,19:0.8" }, "description": "CSV клетка[:alpha] — подсветка с интенсивностью" },
      "Orientation": { "name": "orientation", "in": "query", "schema": { "type": "string", "enum": ["white", "black"], "default": "white" } },
      "LightSquares": { "name": "lightSquares", "in": "query", "schema": { "type": "boolean", "default": false }, "description": "Рабочие клетки светлые (итальянские/испанские)" },
      "AllSquares": { "name": "allSquares", "in": "query", "schema": { "type": "boolean", "default": false }, "description": "Игра на всех клетках (турецкие шашки); все клетки нумеруются" },
      "GameType": { "name": "gameType", "in": "query", "schema": { "type": "integer", "enum": [20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 40, 41, 96, 97, 98] }, "description": "PDN GameType: число (20, 22=italian, 27=canadian 12×12, 30=turkish все клетки, 41=spanciretti 10×8, 96-98=наши) ИЛИ полный тег '21,B,8,8,N1,0' (тип,старт,ширина,высота,нотация+нумерация,инверсия)" },
      "Variant": { "name": "variant", "in": "query", "schema": { "type": "string", "enum": ["standard", "english", "italian", "pool", "spanish", "russian", "brazilian", "canadian", "portuguese", "czech", "turkish", "thai", "frisian", "spanciretti", "breakthrough", "frysk", "antidraughts"] } },
      "Notation": { "name": "notation", "in": "query", "schema": { "type": "string", "enum": ["none", "numeric", "algebraic", "coords"], "default": "none" }, "description": "numeric: номер на каждой нумеруемой клетке (все 1..50 сразу); coords: a1 на каждой клетке" },
      "Numbering": { "name": "numbering", "in": "query", "schema": { "type": "integer", "minimum": 0, "maximum": 3 }, "description": "Абсолютное начало клетки 1: 0 низ-слева, 1 низ-справа, 2 верх-слева (канон движка/FMJD — по умолчанию), 3 верх-справа" },
      "PieceStyle": { "name": "pieceStyle", "in": "query", "schema": { "type": "string", "enum": ["classic", "flat"], "default": "classic" } },
      "Theme": { "name": "theme", "in": "query", "schema": { "type": "string", "enum": ["default", "lidraughts-brown", "lidraughts-blue", "green", "marble", "wood"], "default": "lidraughts-brown" } },
      "White": { "name": "white", "in": "query", "schema": { "type": "string" }, "description": "Имя белых — включает полосу подписи" },
      "Black": { "name": "black", "in": "query", "schema": { "type": "string" } },
      "Comment": { "name": "comment", "in": "query", "schema": { "type": "string" }, "description": "Подпись на полосе; в GIF также пишется в метаданные" },
      "Margin": { "name": "margin", "in": "query", "schema": { "type": "integer", "maximum": 64 } },
      "Transparent": { "name": "transparent", "in": "query", "schema": { "type": "boolean", "default": false } }
    },
    "schemas": {
      "GifRequest": { "type": "object", "required": ["frames"], "properties": {
        "boardSize": { "type": "integer", "default": 10 },
        "boardHeight": { "type": "integer" },
        "lightSquares": { "type": "boolean" },
        "numbering": { "type": "integer", "default": 2 },
        "allSquares": { "type": "boolean" },
        "gameType": { "type": "string", "description": "число или полный PDN-тег '21,B,8,8,N1,0'" },
        "variant": { "type": "string" },
        "size": { "type": "integer", "default": 360, "maximum": 640 },
        "theme": { "type": "string" },
        "flat": { "type": "boolean" },
        "orientation": { "type": "string", "enum": ["white", "black"] },
        "delay_ms": { "type": "integer", "default": 700, "minimum": 100, "maximum": 3000 },
        "white": { "type": "string" },
        "black": { "type": "string" },
        "frames": { "type": "array", "maxItems": 500, "items": { "type": "object", "required": ["fen"], "properties": { "fen": { "type": "string" }, "lastMove": { "type": "string", "example": "4741" } } } }
      } }
    }
  }
}"##.to_string()
}

pub fn swagger_html() -> &'static str {
    r##"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="utf-8">
<title>boardimage — API</title>
<link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
<style>body{margin:0}#topbar{display:none}</style>
</head>
<body>
<div id="ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
<script>
SwaggerUIBundle({ url: '/openapi.json', dom_id: '#ui', deepLinking: true, docExpansion: 'list' });
</script>
</body>
</html>"##
}

pub fn redoc_html() -> &'static str {
    r##"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="utf-8"><title>boardimage — API</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<style>body{margin:0;padding:0}</style>
</head>
<body>
<redoc spec-url="/openapi.json"></redoc>
<script src="https://cdn.redoc.ly/redoc/latest/bundles/redoc.standalone.js"></script>
</body>
</html>"##
}

#[cfg(test)]
mod tests {
    #[test]
    fn spec_is_valid_json() {
        let v: serde_json::Value =
            serde_json::from_str(&super::json()).expect("openapi.json must be valid JSON");
        assert!(v["paths"]["/board.png"].is_object());
        assert!(v["components"]["parameters"]["Fen"]["description"]
            .as_str()
            .unwrap()
            .contains("PDN 3.0"));
        assert!(
            v["components"]["schemas"]["GifRequest"]["properties"]["frames"]["maxItems"] == 500
        );
    }
}
