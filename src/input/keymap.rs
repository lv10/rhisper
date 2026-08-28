// keymap.rs - ASCII to Linux keycode mapping tables for rhisper.
// Mechanical port of keymap.c/keymap.h, extended with a Spanish layout.
// One table per keyboard layout. Letters and digits share physical keys
// across US/Danish/Spanish; only the symbol rows differ.
//
// To add a keyboard layout:
//   1. Write a `fn <name>_overrides() -> Vec<(u8, i32)>` listing only the
//      ASCII chars that differ from the US base table (see danish_overrides
//      for the format).
//   2. Add a ("name", <name>_overrides()) entry in `build_layouts()`.
//   3. The full table is built automatically as base us_map + overrides.
// No changes elsewhere are needed.

use evdev::KeyCode;
use std::sync::LazyLock;

pub const FLAG_UPPERCASE: i32 = i32::MIN; // 0x80000000 - press with Shift
pub const FLAG_ALTGR: i32 = 0x4000_0000; // press with RightAlt (AltGr)
pub const FLAG_DEADKEY: i32 = 0x2000_0000; // dead key: follow with Space

// ISO key left of Z (< > \ on Danish)
pub const KEY_102ND: i32 = 86;

#[inline]
fn k(code: KeyCode) -> i32 {
    code.0 as i32
}

/// US QWERTY mapping. Each entry is either:
///   -1                              (unmapped/unsupported)
///   keycode                         (unshifted character)
///   keycode | FLAG_UPPERCASE        (shifted character)
fn build_us_map() -> [i32; 128] {
    let mut m = [-1i32; 128];

    // Control characters (0x00-0x1f): mostly unmapped except tab and enter
    m[0x09] = k(KeyCode::KEY_TAB);
    m[0x0a] = k(KeyCode::KEY_ENTER);

    // Printable characters (0x20-0x7e)
    // Space and symbols (0x20-0x2f)
    m[b' ' as usize] = k(KeyCode::KEY_SPACE); // 0x20
    m[b'!' as usize] = k(KeyCode::KEY_1) | FLAG_UPPERCASE; // 0x21 (shift+1)
    m[b'"' as usize] = k(KeyCode::KEY_APOSTROPHE) | FLAG_UPPERCASE; // 0x22 (shift+')
    m[b'#' as usize] = k(KeyCode::KEY_3) | FLAG_UPPERCASE; // 0x23 (shift+3)
    m[b'$' as usize] = k(KeyCode::KEY_4) | FLAG_UPPERCASE; // 0x24 (shift+4)
    m[b'%' as usize] = k(KeyCode::KEY_5) | FLAG_UPPERCASE; // 0x25 (shift+5)
    m[b'&' as usize] = k(KeyCode::KEY_7) | FLAG_UPPERCASE; // 0x26 (shift+7)
    m[b'\'' as usize] = k(KeyCode::KEY_APOSTROPHE); // 0x27
    m[b'(' as usize] = k(KeyCode::KEY_9) | FLAG_UPPERCASE; // 0x28 (shift+9)
    m[b')' as usize] = k(KeyCode::KEY_0) | FLAG_UPPERCASE; // 0x29 (shift+0)
    m[b'*' as usize] = k(KeyCode::KEY_8) | FLAG_UPPERCASE; // 0x2a (shift+8)
    m[b'+' as usize] = k(KeyCode::KEY_EQUAL) | FLAG_UPPERCASE; // 0x2b (shift+=)
    m[b',' as usize] = k(KeyCode::KEY_COMMA); // 0x2c
    m[b'-' as usize] = k(KeyCode::KEY_MINUS); // 0x2d
    m[b'.' as usize] = k(KeyCode::KEY_DOT); // 0x2e
    m[b'/' as usize] = k(KeyCode::KEY_SLASH); // 0x2f

    // Digits (0x30-0x39)
    m[b'0' as usize] = k(KeyCode::KEY_0);
    m[b'1' as usize] = k(KeyCode::KEY_1);
    m[b'2' as usize] = k(KeyCode::KEY_2);
    m[b'3' as usize] = k(KeyCode::KEY_3);
    m[b'4' as usize] = k(KeyCode::KEY_4);
    m[b'5' as usize] = k(KeyCode::KEY_5);
    m[b'6' as usize] = k(KeyCode::KEY_6);
    m[b'7' as usize] = k(KeyCode::KEY_7);
    m[b'8' as usize] = k(KeyCode::KEY_8);
    m[b'9' as usize] = k(KeyCode::KEY_9);

    // More symbols (0x3a-0x40)
    m[b':' as usize] = k(KeyCode::KEY_SEMICOLON) | FLAG_UPPERCASE; // (shift+;)
    m[b';' as usize] = k(KeyCode::KEY_SEMICOLON);
    m[b'<' as usize] = k(KeyCode::KEY_COMMA) | FLAG_UPPERCASE; // (shift+,)
    m[b'=' as usize] = k(KeyCode::KEY_EQUAL);
    m[b'>' as usize] = k(KeyCode::KEY_DOT) | FLAG_UPPERCASE; // (shift+.)
    m[b'?' as usize] = k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE; // (shift+/)
    m[b'@' as usize] = k(KeyCode::KEY_2) | FLAG_UPPERCASE; // (shift+2)

    // Uppercase letters (0x41-0x5a): A-Z
    m[b'A' as usize] = k(KeyCode::KEY_A) | FLAG_UPPERCASE;
    m[b'B' as usize] = k(KeyCode::KEY_B) | FLAG_UPPERCASE;
    m[b'C' as usize] = k(KeyCode::KEY_C) | FLAG_UPPERCASE;
    m[b'D' as usize] = k(KeyCode::KEY_D) | FLAG_UPPERCASE;
    m[b'E' as usize] = k(KeyCode::KEY_E) | FLAG_UPPERCASE;
    m[b'F' as usize] = k(KeyCode::KEY_F) | FLAG_UPPERCASE;
    m[b'G' as usize] = k(KeyCode::KEY_G) | FLAG_UPPERCASE;
    m[b'H' as usize] = k(KeyCode::KEY_H) | FLAG_UPPERCASE;
    m[b'I' as usize] = k(KeyCode::KEY_I) | FLAG_UPPERCASE;
    m[b'J' as usize] = k(KeyCode::KEY_J) | FLAG_UPPERCASE;
    m[b'K' as usize] = k(KeyCode::KEY_K) | FLAG_UPPERCASE;
    m[b'L' as usize] = k(KeyCode::KEY_L) | FLAG_UPPERCASE;
    m[b'M' as usize] = k(KeyCode::KEY_M) | FLAG_UPPERCASE;
    m[b'N' as usize] = k(KeyCode::KEY_N) | FLAG_UPPERCASE;
    m[b'O' as usize] = k(KeyCode::KEY_O) | FLAG_UPPERCASE;
    m[b'P' as usize] = k(KeyCode::KEY_P) | FLAG_UPPERCASE;
    m[b'Q' as usize] = k(KeyCode::KEY_Q) | FLAG_UPPERCASE;
    m[b'R' as usize] = k(KeyCode::KEY_R) | FLAG_UPPERCASE;
    m[b'S' as usize] = k(KeyCode::KEY_S) | FLAG_UPPERCASE;
    m[b'T' as usize] = k(KeyCode::KEY_T) | FLAG_UPPERCASE;
    m[b'U' as usize] = k(KeyCode::KEY_U) | FLAG_UPPERCASE;
    m[b'V' as usize] = k(KeyCode::KEY_V) | FLAG_UPPERCASE;
    m[b'W' as usize] = k(KeyCode::KEY_W) | FLAG_UPPERCASE;
    m[b'X' as usize] = k(KeyCode::KEY_X) | FLAG_UPPERCASE;
    m[b'Y' as usize] = k(KeyCode::KEY_Y) | FLAG_UPPERCASE;
    m[b'Z' as usize] = k(KeyCode::KEY_Z) | FLAG_UPPERCASE;

    // Brackets and symbols (0x5b-0x60)
    m[b'[' as usize] = k(KeyCode::KEY_LEFTBRACE);
    m[b'\\' as usize] = k(KeyCode::KEY_BACKSLASH);
    m[b']' as usize] = k(KeyCode::KEY_RIGHTBRACE);
    m[b'^' as usize] = k(KeyCode::KEY_6) | FLAG_UPPERCASE; // (shift+6)
    m[b'_' as usize] = k(KeyCode::KEY_MINUS) | FLAG_UPPERCASE; // (shift+-)
    m[b'`' as usize] = k(KeyCode::KEY_GRAVE);

    // Lowercase letters (0x61-0x7a): a-z
    m[b'a' as usize] = k(KeyCode::KEY_A);
    m[b'b' as usize] = k(KeyCode::KEY_B);
    m[b'c' as usize] = k(KeyCode::KEY_C);
    m[b'd' as usize] = k(KeyCode::KEY_D);
    m[b'e' as usize] = k(KeyCode::KEY_E);
    m[b'f' as usize] = k(KeyCode::KEY_F);
    m[b'g' as usize] = k(KeyCode::KEY_G);
    m[b'h' as usize] = k(KeyCode::KEY_H);
    m[b'i' as usize] = k(KeyCode::KEY_I);
    m[b'j' as usize] = k(KeyCode::KEY_J);
    m[b'k' as usize] = k(KeyCode::KEY_K);
    m[b'l' as usize] = k(KeyCode::KEY_L);
    m[b'm' as usize] = k(KeyCode::KEY_M);
    m[b'n' as usize] = k(KeyCode::KEY_N);
    m[b'o' as usize] = k(KeyCode::KEY_O);
    m[b'p' as usize] = k(KeyCode::KEY_P);
    m[b'q' as usize] = k(KeyCode::KEY_Q);
    m[b'r' as usize] = k(KeyCode::KEY_R);
    m[b's' as usize] = k(KeyCode::KEY_S);
    m[b't' as usize] = k(KeyCode::KEY_T);
    m[b'u' as usize] = k(KeyCode::KEY_U);
    m[b'v' as usize] = k(KeyCode::KEY_V);
    m[b'w' as usize] = k(KeyCode::KEY_W);
    m[b'x' as usize] = k(KeyCode::KEY_X);
    m[b'y' as usize] = k(KeyCode::KEY_Y);
    m[b'z' as usize] = k(KeyCode::KEY_Z);

    // Final symbols (0x7b-0x7e)
    m[b'{' as usize] = k(KeyCode::KEY_LEFTBRACE) | FLAG_UPPERCASE; // (shift+[)
    m[b'|' as usize] = k(KeyCode::KEY_BACKSLASH) | FLAG_UPPERCASE; // (shift+\)
    m[b'}' as usize] = k(KeyCode::KEY_RIGHTBRACE) | FLAG_UPPERCASE; // (shift+])
    m[b'~' as usize] = k(KeyCode::KEY_GRAVE) | FLAG_UPPERCASE; // (shift+`)

    // 0x7f DEL stays unmapped (-1)
    m
}

/// Danish (ISO) differs from US only on the symbol rows; letters and digits
/// sit on the same physical keys. Each override lists one differing ASCII
/// char and its Danish key definition, derived from
/// /usr/share/X11/xkb/symbols/dk:
///   AE11 '+' '?'  AE12 dead_acute/dead_grave + '|' on AltGr
///   AD11 'a-ring' AD12 dead_diaeresis/^/~ on AltGr
///   AC10 'ae'  AC11 'o-slash'  BKSL '\'' '*'
///   LSGT '<' '>' '\' on AltGr
///   AB08 ',' ';'  AB09 '.' ':'  AB10 '-' '_'
/// Non-ASCII keys (a-ring, ae, o-slash, etc.) are typed via clipboard.
fn danish_overrides() -> Vec<(u8, i32)> {
    vec![
        (b'"', k(KeyCode::KEY_2) | FLAG_UPPERCASE), // shift+2
        (b'$', k(KeyCode::KEY_4) | FLAG_ALTGR),     // AltGr+4
        (b'&', k(KeyCode::KEY_6) | FLAG_UPPERCASE), // shift+6
        (b'\'', k(KeyCode::KEY_BACKSLASH)),         // Danish apostrophe key
        (b'(', k(KeyCode::KEY_8) | FLAG_UPPERCASE), // shift+8
        (b')', k(KeyCode::KEY_9) | FLAG_UPPERCASE), // shift+9
        (b'*', k(KeyCode::KEY_BACKSLASH) | FLAG_UPPERCASE), // shift+'
        (b'+', k(KeyCode::KEY_MINUS)),              // Danish + key
        (b'-', k(KeyCode::KEY_SLASH)),              // Danish - key
        (b'/', k(KeyCode::KEY_7) | FLAG_UPPERCASE), // shift+7
        (b':', k(KeyCode::KEY_DOT) | FLAG_UPPERCASE), // shift+.
        (b';', k(KeyCode::KEY_COMMA) | FLAG_UPPERCASE), // shift+,
        (b'<', KEY_102ND),                          // Danish <> key
        (b'=', k(KeyCode::KEY_0) | FLAG_UPPERCASE), // shift+0
        (b'>', KEY_102ND | FLAG_UPPERCASE),         // shift+<>
        (b'?', k(KeyCode::KEY_MINUS) | FLAG_UPPERCASE), // shift++ on AE11
        (b'@', k(KeyCode::KEY_2) | FLAG_ALTGR),     // AltGr+2
        (b'[', k(KeyCode::KEY_8) | FLAG_ALTGR),     // AltGr+8
        (b'\\', KEY_102ND | FLAG_ALTGR),            // AltGr+<>
        (b']', k(KeyCode::KEY_9) | FLAG_ALTGR),     // AltGr+9
        (
            b'^',
            k(KeyCode::KEY_RIGHTBRACE) | FLAG_UPPERCASE | FLAG_DEADKEY,
        ), // dead ^
        (b'_', k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE), // shift+-
        (b'`', k(KeyCode::KEY_EQUAL) | FLAG_UPPERCASE | FLAG_DEADKEY), // dead `
        (b'{', k(KeyCode::KEY_7) | FLAG_ALTGR),     // AltGr+7
        (b'|', k(KeyCode::KEY_EQUAL) | FLAG_ALTGR), // AltGr+´ key
        (b'}', k(KeyCode::KEY_0) | FLAG_ALTGR),     // AltGr+0
        (b'~', k(KeyCode::KEY_RIGHTBRACE) | FLAG_ALTGR | FLAG_DEADKEY), // dead ~
    ]
}

/// Spanish (ISO, "es" "basic" variant) differs from US on most of the
/// symbol keys - derived directly from /usr/share/X11/xkb/symbols/es and
/// the shared latin(type4) base it includes (Spain shares type4 with
/// German/Italian/Portuguese/etc). Physical key positions use the same
/// AEnn/ADnn/ACnn/ABnn/LSGT naming as the X11 symbol files:
///   AE01 '1' '!' '|'(AltGr)         AE02 '2' '"' '@'(AltGr)   (via type4)
///   AE03 '3' (altgr '#')            AE04 '4' '$' '~'(AltGr)   <- tilde lives here
///   AE06 '6' '&'(shift)             AE07 '7' '/'(shift)       (via type4)
///   AE08 '8' '('(shift)             AE09 '9' ')'(shift)       (via type4)
///   AE10 '0' '='(shift)             AE11 '\''  '?'  '\\'(AltGr)
///   AE12 (no ASCII: exclamdown/questiondown/dead_cedilla/dead_ogonek)
///   AD11 dead_grave '`' / dead_circumflex '^' / '['(AltGr)
///   AD12 '+' '*'(shift) / ']'(AltGr)
///   AC11 (AltGr) '{'                BKSL (AltGr) '}'
///   AB08 ','  ';'(shift)            AB09 '.'  ':'(shift)      (via type4)
///   AB10 '-'  '_'(shift)            LSGT '<'  '>'(shift)      (via type4/pc105)
/// Non-ASCII keys (ntilde, ccedilla, masculine, ordfeminine, etc.) are typed
/// via clipboard, same as accented Danish letters.
fn spanish_overrides() -> Vec<(u8, i32)> {
    vec![
        (b'\'', k(KeyCode::KEY_MINUS)),                      // AE11 unshifted
        (b'"', k(KeyCode::KEY_2) | FLAG_UPPERCASE),          // AE02 shift+2
        (b'#', k(KeyCode::KEY_3) | FLAG_ALTGR),              // AE03 AltGr+3
        (b'&', k(KeyCode::KEY_6) | FLAG_UPPERCASE),          // AE06 shift+6
        (b'(', k(KeyCode::KEY_8) | FLAG_UPPERCASE),          // AE08 shift+8
        (b')', k(KeyCode::KEY_9) | FLAG_UPPERCASE),          // AE09 shift+9
        (b'*', k(KeyCode::KEY_RIGHTBRACE) | FLAG_UPPERCASE), // AD12 shift
        (b'+', k(KeyCode::KEY_RIGHTBRACE)),                  // AD12 unshifted
        (b'-', k(KeyCode::KEY_SLASH)),                       // AB10 unshifted
        (b'/', k(KeyCode::KEY_7) | FLAG_UPPERCASE),          // AE07 shift+7
        (b':', k(KeyCode::KEY_DOT) | FLAG_UPPERCASE),        // AB09 shift+.
        (b';', k(KeyCode::KEY_COMMA) | FLAG_UPPERCASE),      // AB08 shift+,
        (b'<', KEY_102ND),                                   // LSGT unshifted
        (b'=', k(KeyCode::KEY_0) | FLAG_UPPERCASE),          // AE10 shift+0
        (b'>', KEY_102ND | FLAG_UPPERCASE),                  // LSGT shift
        (b'?', k(KeyCode::KEY_MINUS) | FLAG_UPPERCASE),      // AE11 shift
        (b'@', k(KeyCode::KEY_2) | FLAG_ALTGR),              // AE02 AltGr+2
        (b'[', k(KeyCode::KEY_LEFTBRACE) | FLAG_ALTGR),      // AD11 AltGr
        (b'\\', k(KeyCode::KEY_MINUS) | FLAG_ALTGR),         // AE11 AltGr
        (b']', k(KeyCode::KEY_RIGHTBRACE) | FLAG_ALTGR),     // AD12 AltGr
        (
            b'^',
            k(KeyCode::KEY_LEFTBRACE) | FLAG_UPPERCASE | FLAG_DEADKEY,
        ), // AD11 shift, dead_circumflex
        (b'_', k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE),      // AB10 shift
        (b'`', k(KeyCode::KEY_LEFTBRACE) | FLAG_DEADKEY),    // AD11 unshifted, dead_grave
        (b'{', k(KeyCode::KEY_APOSTROPHE) | FLAG_ALTGR),     // AC11 AltGr
        (b'|', k(KeyCode::KEY_1) | FLAG_ALTGR),              // AE01 AltGr
        (b'}', k(KeyCode::KEY_BACKSLASH) | FLAG_ALTGR),      // BKSL AltGr
        (b'~', k(KeyCode::KEY_4) | FLAG_ALTGR),              // AE04 AltGr - the tilde fix
    ]
}

/// Registry of supported layouts. Each layout's full table is a copy of the
/// US base table with its sparse overrides applied on top.
fn build_layouts() -> Vec<(&'static str, [i32; 128])> {
    let us = build_us_map();

    let mut dk = us;
    for (c, kdef) in danish_overrides() {
        dk[c as usize] = kdef;
    }

    let mut es = us;
    for (c, kdef) in spanish_overrides() {
        es[c as usize] = kdef;
    }

    vec![("us", us), ("dk", dk), ("es", es)]
}

static LAYOUTS: LazyLock<Vec<(&'static str, [i32; 128])>> = LazyLock::new(build_layouts);

/// Returns keycode | FLAG_* modifiers for the given layout, or -1.
pub fn keymap_lookup(layout: &str, c: u8) -> i32 {
    if c >= 128 {
        return -1;
    }

    for (name, map) in LAYOUTS.iter() {
        if *name == layout {
            return map[c as usize];
        }
    }
    // unknown layout falls back to us (first entry)
    LAYOUTS[0].1[c as usize]
}

#[cfg(test)]
#[allow(clippy::char_lit_as_u8)] // ASCII literals only; matches C test style
mod tests {
    use super::*;

    macro_rules! expect {
        ($layout:expr, $ch:expr, $expected:expr) => {
            assert_eq!(
                keymap_lookup($layout, $ch as u8),
                $expected,
                "keymap_lookup({:?}, {:?})",
                $layout,
                $ch as char
            );
        };
    }

    #[test]
    fn us_layout() {
        expect!("us", 'a', k(KeyCode::KEY_A));
        expect!("us", 'A', k(KeyCode::KEY_A) | FLAG_UPPERCASE);
        expect!("us", '1', k(KeyCode::KEY_1));
        expect!("us", '!', k(KeyCode::KEY_1) | FLAG_UPPERCASE);
        expect!("us", '\'', k(KeyCode::KEY_APOSTROPHE));
        expect!("us", '?', k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE);
        expect!("us", '"', k(KeyCode::KEY_APOSTROPHE) | FLAG_UPPERCASE);
        expect!("us", '@', k(KeyCode::KEY_2) | FLAG_UPPERCASE);
        expect!("us", '^', k(KeyCode::KEY_6) | FLAG_UPPERCASE);
        expect!("us", '~', k(KeyCode::KEY_GRAVE) | FLAG_UPPERCASE);
        expect!("us", '`', k(KeyCode::KEY_GRAVE));
        expect!("us", '\\', k(KeyCode::KEY_BACKSLASH));
        expect!("us", '<', k(KeyCode::KEY_COMMA) | FLAG_UPPERCASE);
        expect!("us", '{', k(KeyCode::KEY_LEFTBRACE) | FLAG_UPPERCASE);
        expect!("us", '|', k(KeyCode::KEY_BACKSLASH) | FLAG_UPPERCASE);
        expect!("us", ' ', k(KeyCode::KEY_SPACE));
    }

    #[test]
    fn dk_layout_reported_bugs() {
        // was being typed as o-slash
        expect!("dk", '\'', k(KeyCode::KEY_BACKSLASH));
        // was being typed as underscore
        expect!("dk", '?', k(KeyCode::KEY_MINUS) | FLAG_UPPERCASE);
    }

    #[test]
    fn dk_layout_letters_and_digits() {
        expect!("dk", 'a', k(KeyCode::KEY_A));
        expect!("dk", 'A', k(KeyCode::KEY_A) | FLAG_UPPERCASE);
        expect!("dk", 'z', k(KeyCode::KEY_Z));
        expect!("dk", 'Z', k(KeyCode::KEY_Z) | FLAG_UPPERCASE);
        expect!("dk", '1', k(KeyCode::KEY_1));
        expect!("dk", '!', k(KeyCode::KEY_1) | FLAG_UPPERCASE);
    }

    #[test]
    fn dk_layout_shifted_symbols() {
        expect!("dk", '"', k(KeyCode::KEY_2) | FLAG_UPPERCASE);
        expect!("dk", '#', k(KeyCode::KEY_3) | FLAG_UPPERCASE);
        expect!("dk", '%', k(KeyCode::KEY_5) | FLAG_UPPERCASE);
        expect!("dk", '&', k(KeyCode::KEY_6) | FLAG_UPPERCASE);
        expect!("dk", '6', k(KeyCode::KEY_6));
        expect!("dk", '/', k(KeyCode::KEY_7) | FLAG_UPPERCASE); // shift+7
        expect!("dk", '7', k(KeyCode::KEY_7));
        expect!("dk", '(', k(KeyCode::KEY_8) | FLAG_UPPERCASE);
        expect!("dk", ')', k(KeyCode::KEY_9) | FLAG_UPPERCASE);
        expect!("dk", '=', k(KeyCode::KEY_0) | FLAG_UPPERCASE);
        expect!("dk", '+', k(KeyCode::KEY_MINUS));
        expect!("dk", '*', k(KeyCode::KEY_BACKSLASH) | FLAG_UPPERCASE);
        expect!("dk", ';', k(KeyCode::KEY_COMMA) | FLAG_UPPERCASE);
        expect!("dk", ':', k(KeyCode::KEY_DOT) | FLAG_UPPERCASE);
        expect!("dk", '<', KEY_102ND);
        expect!("dk", '>', KEY_102ND | FLAG_UPPERCASE);
        expect!("dk", '-', k(KeyCode::KEY_SLASH));
        expect!("dk", '_', k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE);
    }

    #[test]
    fn dk_layout_altgr() {
        expect!("dk", '@', k(KeyCode::KEY_2) | FLAG_ALTGR);
        expect!("dk", '$', k(KeyCode::KEY_4) | FLAG_ALTGR);
        expect!("dk", '{', k(KeyCode::KEY_7) | FLAG_ALTGR);
        expect!("dk", '[', k(KeyCode::KEY_8) | FLAG_ALTGR);
        expect!("dk", ']', k(KeyCode::KEY_9) | FLAG_ALTGR);
        expect!("dk", '}', k(KeyCode::KEY_0) | FLAG_ALTGR);
        expect!("dk", '\\', KEY_102ND | FLAG_ALTGR);
        expect!("dk", '|', k(KeyCode::KEY_EQUAL) | FLAG_ALTGR);
    }

    #[test]
    fn dk_layout_dead_keys() {
        expect!(
            "dk",
            '`',
            k(KeyCode::KEY_EQUAL) | FLAG_UPPERCASE | FLAG_DEADKEY
        );
        expect!(
            "dk",
            '^',
            k(KeyCode::KEY_RIGHTBRACE) | FLAG_UPPERCASE | FLAG_DEADKEY
        );
        expect!(
            "dk",
            '~',
            k(KeyCode::KEY_RIGHTBRACE) | FLAG_ALTGR | FLAG_DEADKEY
        );
    }

    /* ── Spanish layout: the reported bug (tilde) ────────────────────────── */

    #[test]
    fn es_layout_tilde_bug() {
        // The C version's Spanish users fell back to the "us" table, which
        // maps '~' to Shift+Grave - on a real Spanish system layout that key
        // combo produces "ª" (ordfeminine), not a tilde. Fixed via AltGr+4,
        // which is what a real Spanish keyboard's basic layout assigns to a
        // literal (non-dead) tilde.
        expect!("es", '~', k(KeyCode::KEY_4) | FLAG_ALTGR);
    }

    #[test]
    fn es_layout_letters_and_digits() {
        expect!("es", 'a', k(KeyCode::KEY_A));
        expect!("es", 'A', k(KeyCode::KEY_A) | FLAG_UPPERCASE);
        expect!("es", 'n', k(KeyCode::KEY_N));
        expect!("es", '5', k(KeyCode::KEY_5));
    }

    #[test]
    fn es_layout_shifted_symbols() {
        expect!("es", '"', k(KeyCode::KEY_2) | FLAG_UPPERCASE);
        expect!("es", '&', k(KeyCode::KEY_6) | FLAG_UPPERCASE);
        expect!("es", '/', k(KeyCode::KEY_7) | FLAG_UPPERCASE);
        expect!("es", '(', k(KeyCode::KEY_8) | FLAG_UPPERCASE);
        expect!("es", ')', k(KeyCode::KEY_9) | FLAG_UPPERCASE);
        expect!("es", '=', k(KeyCode::KEY_0) | FLAG_UPPERCASE);
        expect!("es", '\'', k(KeyCode::KEY_MINUS));
        expect!("es", '?', k(KeyCode::KEY_MINUS) | FLAG_UPPERCASE);
        expect!("es", '+', k(KeyCode::KEY_RIGHTBRACE));
        expect!("es", '*', k(KeyCode::KEY_RIGHTBRACE) | FLAG_UPPERCASE);
        expect!("es", ';', k(KeyCode::KEY_COMMA) | FLAG_UPPERCASE);
        expect!("es", ':', k(KeyCode::KEY_DOT) | FLAG_UPPERCASE);
        expect!("es", '-', k(KeyCode::KEY_SLASH));
        expect!("es", '_', k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE);
        expect!("es", '<', KEY_102ND);
        expect!("es", '>', KEY_102ND | FLAG_UPPERCASE);
    }

    #[test]
    fn es_layout_altgr() {
        expect!("es", '@', k(KeyCode::KEY_2) | FLAG_ALTGR);
        expect!("es", '#', k(KeyCode::KEY_3) | FLAG_ALTGR);
        expect!("es", '[', k(KeyCode::KEY_LEFTBRACE) | FLAG_ALTGR);
        expect!("es", ']', k(KeyCode::KEY_RIGHTBRACE) | FLAG_ALTGR);
        expect!("es", '{', k(KeyCode::KEY_APOSTROPHE) | FLAG_ALTGR);
        expect!("es", '}', k(KeyCode::KEY_BACKSLASH) | FLAG_ALTGR);
        expect!("es", '\\', k(KeyCode::KEY_MINUS) | FLAG_ALTGR);
        expect!("es", '|', k(KeyCode::KEY_1) | FLAG_ALTGR);
    }

    #[test]
    fn es_layout_dead_keys() {
        expect!("es", '`', k(KeyCode::KEY_LEFTBRACE) | FLAG_DEADKEY);
        expect!(
            "es",
            '^',
            k(KeyCode::KEY_LEFTBRACE) | FLAG_UPPERCASE | FLAG_DEADKEY
        );
    }

    #[test]
    fn non_ascii_falls_back_to_clipboard() {
        // Non-ASCII falls back to the clipboard path in rhisper's paste logic.
        assert_eq!(keymap_lookup("us", 0xC3), -1);
        assert_eq!(keymap_lookup("dk", 0xE6), -1);
        assert_eq!(keymap_lookup("es", 0xF1), -1); // ñ
    }

    #[test]
    fn unknown_layout_falls_back_to_us() {
        expect!("gibberish", '\'', k(KeyCode::KEY_APOSTROPHE));
        expect!("gibberish", '?', k(KeyCode::KEY_SLASH) | FLAG_UPPERCASE);
    }

    // Printable-ASCII classification logic layered on top of keymap_lookup().

    #[test]
    fn printable_ascii_range_is_mapped() {
        for c in 32u8..=126u8 {
            assert_ne!(
                keymap_lookup("us", c),
                -1,
                "printable ASCII char {c} is mapped"
            );
        }
    }

    #[test]
    fn control_chars_are_unmapped() {
        for c in 0u8..=8u8 {
            assert_eq!(keymap_lookup("us", c), -1, "control char {c:#x} unmapped");
        }
        for c in 11u8..=31u8 {
            assert_eq!(keymap_lookup("us", c), -1, "control char {c:#x} unmapped");
        }
    }

    #[test]
    fn tab_and_enter_are_mapped() {
        assert_ne!(keymap_lookup("us", b'\t'), -1);
        assert_ne!(keymap_lookup("us", b'\n'), -1);
    }

    #[test]
    fn del_is_unmapped() {
        assert_eq!(keymap_lookup("us", 127), -1);
    }

    #[test]
    fn uppercase_letters_have_shift_flag() {
        for c in b'A'..=b'Z' {
            assert_ne!(keymap_lookup("us", c) & FLAG_UPPERCASE, 0);
        }
    }

    #[test]
    fn lowercase_letters_have_no_shift_flag() {
        for c in b'a'..=b'z' {
            assert_eq!(keymap_lookup("us", c) & FLAG_UPPERCASE, 0);
        }
    }

    #[test]
    fn digits_have_no_shift_flag() {
        for c in b'0'..=b'9' {
            assert_eq!(keymap_lookup("us", c) & FLAG_UPPERCASE, 0);
        }
    }

    #[test]
    fn shifted_symbols_have_shift_flag() {
        for c in "!@#$%^&*()_+{}|:\"<>?~".bytes() {
            assert_ne!(keymap_lookup("us", c) & FLAG_UPPERCASE, 0, "{}", c as char);
        }
    }

    #[test]
    fn unshifted_symbols_have_no_shift_flag() {
        for c in "`-=[]\\;',./".bytes() {
            assert_eq!(keymap_lookup("us", c) & FLAG_UPPERCASE, 0, "{}", c as char);
        }
    }
}
