use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static THEME_COUNTER: AtomicUsize = AtomicUsize::new(usize::MAX);

#[derive(Clone, Copy, Debug)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb(r, g, b)
    }

    pub fn fg(&self, text: &str) -> String {
        format!("\x1b[38;2;{};{};{}m{}\x1b[0m", self.0, self.1, self.2, text)
    }

    pub fn bold(&self, text: &str) -> String {
        format!("\x1b[1m\x1b[38;2;{};{};{}m{}\x1b[0m", self.0, self.1, self.2, text)
    }

    pub fn interpolate(c1: Rgb, c2: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let r = ((c1.0 as f32) * (1.0 - t) + (c2.0 as f32) * t).round() as u8;
        let g = ((c1.1 as f32) * (1.0 - t) + (c2.1 as f32) * t).round() as u8;
        let b = ((c1.2 as f32) * (1.0 - t) + (c2.2 as f32) * t).round() as u8;
        Rgb(r, g, b)
    }
}

pub struct Theme {
    pub name: &'static str,
    pub c1: Rgb,
    pub c2: Rgb,
    pub c3: Rgb,
    pub accent: Rgb,
    pub border: Rgb,
    pub muted: Rgb,
}

pub static PALETTES: &[Theme] = &[
    Theme {
        name: "Cyberpunk Neon",
        c1: Rgb::new(0, 240, 255),
        c2: Rgb::new(255, 0, 128),
        c3: Rgb::new(155, 77, 255),
        accent: Rgb::new(255, 230, 0),
        border: Rgb::new(0, 180, 216),
        muted: Rgb::new(140, 175, 210),
    },
    Theme {
        name: "Synthwave Sunset",
        c1: Rgb::new(255, 60, 140),
        c2: Rgb::new(255, 130, 45),
        c3: Rgb::new(255, 215, 0),
        accent: Rgb::new(0, 255, 210),
        border: Rgb::new(255, 80, 130),
        muted: Rgb::new(215, 160, 185),
    },
    Theme {
        name: "Matrix Emerald",
        c1: Rgb::new(57, 255, 20),
        c2: Rgb::new(0, 255, 170),
        c3: Rgb::new(0, 215, 255),
        accent: Rgb::new(255, 255, 255),
        border: Rgb::new(0, 200, 110),
        muted: Rgb::new(120, 195, 150),
    },
    Theme {
        name: "Deep Oceanic",
        c1: Rgb::new(0, 195, 255),
        c2: Rgb::new(60, 120, 255),
        c3: Rgb::new(150, 60, 240),
        accent: Rgb::new(52, 211, 153),
        border: Rgb::new(30, 144, 255),
        muted: Rgb::new(145, 180, 225),
    },
    Theme {
        name: "Solar Flare",
        c1: Rgb::new(255, 70, 70),
        c2: Rgb::new(255, 145, 0),
        c3: Rgb::new(255, 220, 50),
        accent: Rgb::new(255, 255, 120),
        border: Rgb::new(255, 105, 30),
        muted: Rgb::new(220, 165, 140),
    },
    Theme {
        name: "Aurora Borealis",
        c1: Rgb::new(74, 222, 128),
        c2: Rgb::new(45, 212, 191),
        c3: Rgb::new(168, 85, 247),
        accent: Rgb::new(250, 204, 21),
        border: Rgb::new(20, 184, 166),
        muted: Rgb::new(150, 205, 190),
    },
    Theme {
        name: "Tokyo Night",
        c1: Rgb::new(244, 114, 182),
        c2: Rgb::new(129, 140, 248),
        c3: Rgb::new(56, 189, 248),
        accent: Rgb::new(251, 146, 60),
        border: Rgb::new(167, 139, 250),
        muted: Rgb::new(185, 170, 215),
    },
    Theme {
        name: "Laser Cyber",
        c1: Rgb::new(255, 0, 110),
        c2: Rgb::new(255, 190, 0),
        c3: Rgb::new(0, 255, 200),
        accent: Rgb::new(255, 255, 255),
        border: Rgb::new(255, 110, 0),
        muted: Rgb::new(205, 190, 185),
    },
    Theme {
        name: "Galactic Nebula",
        c1: Rgb::new(99, 102, 241),
        c2: Rgb::new(168, 85, 247),
        c3: Rgb::new(236, 72, 153),
        accent: Rgb::new(103, 232, 249),
        border: Rgb::new(139, 92, 246),
        muted: Rgb::new(180, 180, 220),
    },
    Theme {
        name: "Electrum Gold",
        c1: Rgb::new(245, 158, 11),
        c2: Rgb::new(251, 191, 36),
        c3: Rgb::new(254, 243, 199),
        accent: Rgb::new(56, 189, 248),
        border: Rgb::new(217, 119, 6),
        muted: Rgb::new(210, 190, 150),
    },
];

fn current_or_init_index() -> usize {
    let val = THEME_COUNTER.load(Ordering::Relaxed);
    if val == usize::MAX {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| (d.subsec_nanos() ^ (d.as_secs() as u32)) as usize)
            .unwrap_or(0);
        let initial = seed % PALETTES.len();
        THEME_COUNTER.store(initial, Ordering::Relaxed);
        initial
    } else {
        val
    }
}

pub fn next_theme() -> &'static Theme {
    let current = current_or_init_index();
    let next = (current + 1) % PALETTES.len();
    THEME_COUNTER.store(next, Ordering::Relaxed);
    &PALETTES[next]
}

pub fn current_theme() -> &'static Theme {
    let idx = current_or_init_index();
    &PALETTES[idx % PALETTES.len()]
}

/// Applies a smooth 3-stop horizontal RGB gradient across a line of text
pub fn gradient_3stop(text: &str, c1: Rgb, c2: Rgb, c3: Rgb) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n <= 1 {
        return c1.fg(text);
    }

    let mut out = String::with_capacity(text.len() * 20);
    for (i, &ch) in chars.iter().enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let color = if t < 0.5 {
            Rgb::interpolate(c1, c2, t * 2.0)
        } else {
            Rgb::interpolate(c2, c3, (t - 0.5) * 2.0)
        };
        out.push_str(&color.fg(&ch.to_string()));
    }
    out
}

/// Applies a smooth 2-stop horizontal gradient across a line of text
pub fn gradient_2stop(text: &str, c1: Rgb, c2: Rgb) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n <= 1 {
        return c1.fg(text);
    }

    let mut out = String::with_capacity(text.len() * 20);
    for (i, &ch) in chars.iter().enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let color = Rgb::interpolate(c1, c2, t);
        out.push_str(&color.fg(&ch.to_string()));
    }
    out
}

/// Render multi-line ASCII art with a smooth 2D diagonal RGB gradient
pub fn render_ascii_art_gradient(lines: &[&str], c1: Rgb, c2: Rgb, c3: Rgb) -> Vec<String> {
    let total_rows = lines.len();
    let max_cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1);

    lines
        .iter()
        .enumerate()
        .map(|(row, line)| {
            let mut out = String::with_capacity(line.len() * 20);
            for (col, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    out.push(' ');
                    continue;
                }
                let t_row = if total_rows > 1 { row as f32 / (total_rows - 1) as f32 } else { 0.0 };
                let t_col = if max_cols > 1 { col as f32 / (max_cols - 1) as f32 } else { 0.0 };
                let t = (t_col * 0.75 + t_row * 0.25).clamp(0.0, 1.0);

                let color = if t < 0.5 {
                    Rgb::interpolate(c1, c2, t * 2.0)
                } else {
                    Rgb::interpolate(c2, c3, (t - 0.5) * 2.0)
                };
                out.push_str(&color.bold(&ch.to_string()));
            }
            out
        })
        .collect()
}

// Status tag helpers (pure ASCII, high vibrancy)
pub fn tag_ok() -> String {
    Rgb::new(50, 255, 120).bold("[+]")
}

pub fn tag_info() -> String {
    Rgb::new(0, 215, 255).bold("[*]")
}

pub fn tag_prompt() -> String {
    Rgb::new(255, 215, 0).bold("[?]")
}

pub fn tag_warn() -> String {
    Rgb::new(255, 140, 0).bold("[!]")
}

pub fn tag_err() -> String {
    Rgb::new(255, 60, 60).bold("[ERROR]")
}

pub fn tag_success() -> String {
    Rgb::new(50, 255, 120).bold("[SUCCESS]")
}

#[cfg(windows)]
pub fn enable_ansi_support() {
    extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
        fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

#[cfg(not(windows))]
pub fn enable_ansi_support() {}
