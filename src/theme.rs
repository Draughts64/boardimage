// Board themes (SPEC.md §4.4).

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub light: [u8; 3],
    pub dark: [u8; 3],
    pub last_light: [u8; 3],
    pub last_dark: [u8; 3],
}

const LAST: [u8; 3] = [0xcd, 0xd2, 0x6a];
const LAST_DARK: [u8; 3] = [0xaa, 0xa2, 0x3b];

pub const THEMES: &[Theme] = &[
    Theme {
        name: "default",
        light: [0xf0, 0xd9, 0xb5],
        dark: [0xb5, 0x88, 0x63],
        last_light: LAST,
        last_dark: LAST_DARK,
    },
    Theme {
        name: "lidraughts-brown",
        light: [0xff, 0xce, 0x9e],
        dark: [0xd1, 0x8b, 0x47],
        last_light: LAST,
        last_dark: LAST_DARK,
    },
    Theme {
        name: "lidraughts-blue",
        light: [0xde, 0xe3, 0xe6],
        dark: [0x8c, 0xa2, 0xad],
        last_light: LAST,
        last_dark: LAST_DARK,
    },
    Theme {
        name: "green",
        light: [0xff, 0xff, 0xdd],
        dark: [0x86, 0xa6, 0x66],
        last_light: LAST,
        last_dark: LAST_DARK,
    },
    Theme {
        name: "marble",
        light: [0xe8, 0xe8, 0xe8],
        dark: [0x99, 0x99, 0x99],
        last_light: LAST,
        last_dark: LAST_DARK,
    },
    Theme {
        name: "wood",
        light: [0xd0, 0xb0, 0x78],
        dark: [0x84, 0x56, 0x33],
        last_light: LAST,
        last_dark: LAST_DARK,
    },
];

pub fn by_name(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.name == name)
}

pub fn default_theme() -> &'static Theme {
    by_name("lidraughts-brown").unwrap()
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
