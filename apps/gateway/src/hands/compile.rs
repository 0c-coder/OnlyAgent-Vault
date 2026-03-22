//! Macro instruction compiler.
//!
//! Compiles high-level macro instructions (e.g., "open_browser", "navigate_url")
//! into raw HID keystroke sequences specific to the target OS.

use super::models::*;

/// Compile a single macro instruction into a sequence of raw keystrokes.
pub fn compile_macro(instruction: &MacroInstruction, os: &HostOS) -> Vec<RawKeystroke> {
    match instruction {
        MacroInstruction::OpenBrowser { browser } => {
            compile_open_browser(os, browser.as_deref())
        }
        MacroInstruction::NavigateUrl { url } => compile_navigate_url(os, url),
        MacroInstruction::OpenTerminal => compile_open_terminal(os),
        MacroInstruction::RunCommand { command } => compile_run_command(os, command),
        MacroInstruction::Screenshot => compile_screenshot(os),
        MacroInstruction::SwitchWindow => compile_switch_window(os),
        MacroInstruction::CloseWindow => compile_close_window(os),
        MacroInstruction::SelectAll => compile_select_all(os),
        MacroInstruction::Copy => compile_copy(os),
        MacroInstruction::Paste => compile_paste(os),
        MacroInstruction::Save => compile_save(os),
        MacroInstruction::Undo => compile_undo(os),
        MacroInstruction::Find { text } => compile_find(os, text),
        MacroInstruction::TypeText { text } => compile_type_text(text),
        MacroInstruction::Wait { seconds } => vec![RawKeystroke::Delay { ms: seconds * 1000 }],
        MacroInstruction::KeyPress { key, modifiers } => {
            compile_key_press(key, modifiers.as_deref())
        }
        MacroInstruction::KeyCombo { keys, modifiers } => {
            compile_key_combo(keys, modifiers.as_deref())
        }
    }
}

/// Compile a full instruction set (list of macros) for a target OS.
pub fn compile_instruction_set(macros: &[MacroInstruction], os: &HostOS) -> Vec<RawKeystroke> {
    macros.iter().flat_map(|m| compile_macro(m, os)).collect()
}

// ── Individual macro compilers ─────────────────────────────────────────

fn gui_mod(os: &HostOS) -> u8 {
    match os {
        HostOS::MacOS => MOD_GUI,
        _ => MOD_CTRL,
    }
}

fn compile_open_browser(os: &HostOS, browser: Option<&str>) -> Vec<RawKeystroke> {
    let app_name = browser.unwrap_or("chrome");
    match os {
        HostOS::MacOS => vec![
            // Cmd+Space → Spotlight
            RawKeystroke::Combo { codes: vec![KEY_SPACE], mods: Some(MOD_GUI) },
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::TypeText { text: app_name.to_string(), delay_per_char_ms: Some(30) },
            RawKeystroke::Delay { ms: 300 },
            RawKeystroke::Key { code: KEY_ENTER, mods: None },
            RawKeystroke::Delay { ms: 1500 },
        ],
        HostOS::Windows => vec![
            // Win key → Start menu search
            RawKeystroke::Key { code: KEY_SPACE, mods: Some(MOD_GUI) },
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::TypeText { text: app_name.to_string(), delay_per_char_ms: Some(30) },
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::Key { code: KEY_ENTER, mods: None },
            RawKeystroke::Delay { ms: 2000 },
        ],
        HostOS::Linux => vec![
            // Super key → App launcher
            RawKeystroke::Key { code: KEY_SPACE, mods: Some(MOD_GUI) },
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::TypeText { text: app_name.to_string(), delay_per_char_ms: Some(30) },
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::Key { code: KEY_ENTER, mods: None },
            RawKeystroke::Delay { ms: 2000 },
        ],
    }
}

fn compile_navigate_url(os: &HostOS, url: &str) -> Vec<RawKeystroke> {
    let mod_key = gui_mod(os);
    vec![
        // Ctrl/Cmd+L → Focus address bar
        RawKeystroke::Combo { codes: vec![KEY_L], mods: Some(mod_key) },
        RawKeystroke::Delay { ms: 200 },
        // Select all existing text
        RawKeystroke::Combo { codes: vec![KEY_A], mods: Some(mod_key) },
        RawKeystroke::Delay { ms: 100 },
        // Type URL
        RawKeystroke::TypeText { text: url.to_string(), delay_per_char_ms: Some(10) },
        RawKeystroke::Key { code: KEY_ENTER, mods: None },
        RawKeystroke::Delay { ms: 2000 },
    ]
}

fn compile_open_terminal(os: &HostOS) -> Vec<RawKeystroke> {
    match os {
        HostOS::MacOS => vec![
            RawKeystroke::Combo { codes: vec![KEY_SPACE], mods: Some(MOD_GUI) },
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::TypeText { text: "terminal".to_string(), delay_per_char_ms: Some(30) },
            RawKeystroke::Delay { ms: 300 },
            RawKeystroke::Key { code: KEY_ENTER, mods: None },
            RawKeystroke::Delay { ms: 1000 },
        ],
        HostOS::Windows => vec![
            // Win+R → Run dialog
            RawKeystroke::Combo { codes: vec![0x15], mods: Some(MOD_GUI) }, // 0x15 = 'r'
            RawKeystroke::Delay { ms: 500 },
            RawKeystroke::TypeText { text: "cmd".to_string(), delay_per_char_ms: Some(30) },
            RawKeystroke::Key { code: KEY_ENTER, mods: None },
            RawKeystroke::Delay { ms: 1000 },
        ],
        HostOS::Linux => vec![
            // Ctrl+Alt+T → Terminal
            RawKeystroke::Combo {
                codes: vec![KEY_T],
                mods: Some(MOD_CTRL | MOD_ALT),
            },
            RawKeystroke::Delay { ms: 1000 },
        ],
    }
}

fn compile_run_command(os: &HostOS, command: &str) -> Vec<RawKeystroke> {
    let mut keystrokes = compile_open_terminal(os);
    keystrokes.push(RawKeystroke::TypeText {
        text: command.to_string(),
        delay_per_char_ms: Some(10),
    });
    keystrokes.push(RawKeystroke::Key { code: KEY_ENTER, mods: None });
    keystrokes.push(RawKeystroke::Delay { ms: 1000 });
    keystrokes
}

fn compile_screenshot(os: &HostOS) -> Vec<RawKeystroke> {
    match os {
        HostOS::MacOS => vec![
            // Cmd+Shift+3 → Full screen capture
            RawKeystroke::Combo {
                codes: vec![0x20], // '3'
                mods: Some(MOD_GUI | MOD_SHIFT),
            },
            RawKeystroke::Delay { ms: 500 },
        ],
        HostOS::Windows => vec![
            // Win+Shift+S → Snipping tool
            RawKeystroke::Combo {
                codes: vec![0x16], // 's'
                mods: Some(MOD_GUI | MOD_SHIFT),
            },
            RawKeystroke::Delay { ms: 500 },
        ],
        HostOS::Linux => vec![
            RawKeystroke::Key { code: KEY_PRINTSCREEN, mods: None },
            RawKeystroke::Delay { ms: 500 },
        ],
    }
}

fn compile_switch_window(os: &HostOS) -> Vec<RawKeystroke> {
    match os {
        HostOS::MacOS => vec![
            RawKeystroke::Combo { codes: vec![KEY_TAB], mods: Some(MOD_GUI) },
            RawKeystroke::Delay { ms: 300 },
        ],
        _ => vec![
            RawKeystroke::Combo { codes: vec![KEY_TAB], mods: Some(MOD_ALT) },
            RawKeystroke::Delay { ms: 300 },
        ],
    }
}

fn compile_close_window(os: &HostOS) -> Vec<RawKeystroke> {
    let mod_key = gui_mod(os);
    vec![
        RawKeystroke::Combo {
            codes: vec![0x1A], // 'w'
            mods: Some(mod_key),
        },
        RawKeystroke::Delay { ms: 300 },
    ]
}

fn compile_select_all(os: &HostOS) -> Vec<RawKeystroke> {
    vec![RawKeystroke::Combo {
        codes: vec![KEY_A],
        mods: Some(gui_mod(os)),
    }]
}

fn compile_copy(os: &HostOS) -> Vec<RawKeystroke> {
    vec![RawKeystroke::Combo {
        codes: vec![0x06], // 'c'
        mods: Some(gui_mod(os)),
    }]
}

fn compile_paste(os: &HostOS) -> Vec<RawKeystroke> {
    vec![RawKeystroke::Combo {
        codes: vec![0x19], // 'v'
        mods: Some(gui_mod(os)),
    }]
}

fn compile_save(os: &HostOS) -> Vec<RawKeystroke> {
    vec![RawKeystroke::Combo {
        codes: vec![0x16], // 's'
        mods: Some(gui_mod(os)),
    }]
}

fn compile_undo(os: &HostOS) -> Vec<RawKeystroke> {
    vec![RawKeystroke::Combo {
        codes: vec![0x1D], // 'z'
        mods: Some(gui_mod(os)),
    }]
}

fn compile_find(os: &HostOS, text: &str) -> Vec<RawKeystroke> {
    let mod_key = gui_mod(os);
    vec![
        RawKeystroke::Combo {
            codes: vec![0x09], // 'f'
            mods: Some(mod_key),
        },
        RawKeystroke::Delay { ms: 200 },
        RawKeystroke::TypeText { text: text.to_string(), delay_per_char_ms: Some(10) },
        RawKeystroke::Key { code: KEY_ENTER, mods: None },
    ]
}

fn compile_type_text(text: &str) -> Vec<RawKeystroke> {
    vec![RawKeystroke::TypeText {
        text: text.to_string(),
        delay_per_char_ms: Some(5),
    }]
}

fn compile_key_press(key: &str, _modifiers: Option<&[String]>) -> Vec<RawKeystroke> {
    let code = key_name_to_code(key);
    vec![RawKeystroke::Key { code, mods: None }]
}

fn compile_key_combo(keys: &[String], _modifiers: Option<&[String]>) -> Vec<RawKeystroke> {
    let codes: Vec<u8> = keys.iter().map(|k| key_name_to_code(k)).collect();
    vec![RawKeystroke::Combo { codes, mods: None }]
}

/// Map common key names to HID usage codes.
fn key_name_to_code(name: &str) -> u8 {
    match name.to_lowercase().as_str() {
        "enter" | "return" => KEY_ENTER,
        "escape" | "esc" => KEY_ESCAPE,
        "space" => KEY_SPACE,
        "tab" => KEY_TAB,
        "backspace" => KEY_BACKSPACE,
        "delete" | "del" => KEY_DELETE,
        "up" => KEY_UP,
        "down" => KEY_DOWN,
        "left" => KEY_LEFT,
        "right" => KEY_RIGHT,
        "f1" => KEY_F1,
        "f3" => KEY_F3,
        "f5" => KEY_F5,
        "printscreen" => KEY_PRINTSCREEN,
        // Letters: a=0x04, b=0x05, ..., z=0x1D
        s if s.len() == 1 => {
            let ch = s.as_bytes()[0];
            if ch.is_ascii_lowercase() {
                0x04 + (ch - b'a')
            } else if ch.is_ascii_digit() {
                if ch == b'0' { 0x27 } else { 0x1E + (ch - b'1') }
            } else {
                0x00 // unknown
            }
        }
        _ => 0x00,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_open_browser_macos() {
        let result = compile_macro(
            &MacroInstruction::OpenBrowser { browser: Some("firefox".into()) },
            &HostOS::MacOS,
        );
        assert!(result.len() >= 4);
        // First instruction should be Cmd+Space
        match &result[0] {
            RawKeystroke::Combo { mods: Some(m), .. } => assert_eq!(*m, MOD_GUI),
            _ => panic!("expected Combo"),
        }
    }

    #[test]
    fn compile_navigate_url_windows() {
        let result = compile_macro(
            &MacroInstruction::NavigateUrl { url: "https://example.com".into() },
            &HostOS::Windows,
        );
        // Should include Ctrl+L, select all, type URL, enter
        assert!(result.len() >= 4);
        match &result[0] {
            RawKeystroke::Combo { mods: Some(m), .. } => assert_eq!(*m, MOD_CTRL),
            _ => panic!("expected Combo with Ctrl"),
        }
    }

    #[test]
    fn compile_type_text_simple() {
        let result = compile_macro(
            &MacroInstruction::TypeText { text: "hello".into() },
            &HostOS::Linux,
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            RawKeystroke::TypeText { text, .. } => assert_eq!(text, "hello"),
            _ => panic!("expected TypeText"),
        }
    }

    #[test]
    fn compile_wait() {
        let result = compile_macro(
            &MacroInstruction::Wait { seconds: 5 },
            &HostOS::MacOS,
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            RawKeystroke::Delay { ms } => assert_eq!(*ms, 5000),
            _ => panic!("expected Delay"),
        }
    }

    #[test]
    fn key_name_to_code_letters() {
        assert_eq!(key_name_to_code("a"), 0x04);
        assert_eq!(key_name_to_code("z"), 0x1D);
        assert_eq!(key_name_to_code("enter"), KEY_ENTER);
        assert_eq!(key_name_to_code("space"), KEY_SPACE);
    }
}
