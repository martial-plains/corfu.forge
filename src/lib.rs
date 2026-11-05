// SPDX-License-Identifier: GPL-3.0-or-later
//! corfu for Forge: completion at point in a popup, opening by itself
//! once typing pauses after a few characters (corfu-auto). C-n / C-p (or
//! the arrows) move, TAB or RET inserts, C-g closes, and anything else
//! keeps typing. M-TAB (or C-M-i) opens it by hand.
//!
//! The popup itself is the editor's, so it looks and works the same in
//! the terminal and the app. Forge also has the other commands
//! (corfu-first, -last, -scroll-up, -scroll-down, -send, -reset, -quit,
//! -quick-insert, -quick-jump) and the modes corfu-indexed-mode,
//! corfu-echo-mode, corfu-history-mode and corfu-mouse-mode; those, and
//! `forge.opt["corfu_"]*`, work with or without this plugin loaded. This
//! plugin adds the keyword source, corfu-popupinfo-mode, corfu-minibuffer-mode
//! and the settings commands below.
//!
//! Candidates: words from the open buffers (built in), this plugin's
//! language keywords (like cape-keyword), and other plugins' sources (the
//! lsp plugin's, for one). Forge filters and draws; sources are asked
//! once per completion.

use forge_plugin_sdk::{
    Buffer, Options, Position, Selection, Span, accept, arm_quick, bind_in_prompt, candidate,
    command, command_exists, completion, completion_source, expand, first, forge_plugin,
    insert_separator, last, locate, message, next, popup_info_beginning, popup_info_end, previous,
    quit, reset, run_command, scroll, scroll_popup_info, selected, set_options, set_popup_frame,
    set_popup_info, set_sorting, toggle_popup_info,
};
use std::cell::{Cell, RefCell};

/// Corfu's defaults (`corfu-auto-delay`, `corfu-auto-prefix`,
/// `corfu-count`, `corfu-min-width`, `corfu-max-width`).
const DEFAULT_DELAY_MS: u32 = 200;
const DEFAULT_MIN_PREFIX: u32 = 3;
const DEFAULT_LINES: u32 = 10;
const DEFAULT_MIN_WIDTH: u32 = 15;
const DEFAULT_MAX_WIDTH: u32 = 100;

/// corfu-popupinfo's defaults (`corfu-popupinfo-delay`'s second value,
/// `corfu-popupinfo-max-width`, `corfu-popupinfo-max-height`).
const DEFAULT_INFO_DELAY_MS: u32 = 500;
const DEFAULT_INFO_MAX_WIDTH: u32 = 80;
const DEFAULT_INFO_MAX_HEIGHT: u32 = 10;

/// The popup's prompt map, the one in charge while a popup is open. Forge
/// owns the editing keys in it; corfu adds to it rather than taking any.
const POPUP: &str = "completion-popup";

/// The prompt map in charge while the vertical list is up (the one
/// corfu-minibuffer-mode adds its list keys to).
const LIST: &str = "completion-list";

/// `corfu-sort-length-alpha`, corfu's default order.
const DEFAULT_SORT: &str = "length";

/// What `M-x corfu-sort` cycles through.
const SORTS: &[&str] = &["length", "alpha", "history", "history-length", "position"];

/// Keywords (and a few common names) per major mode.
pub const KEYWORDS: &[(&str, &[&str])] = &[
    (
        "rust-mode",
        &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while", "Option", "Result", "Some",
            "None", "Ok", "Err", "String", "Vec", "Box", "println",
        ],
    ),
    (
        "python-mode",
        &[
            "False",
            "None",
            "True",
            "and",
            "as",
            "assert",
            "async",
            "await",
            "break",
            "class",
            "continue",
            "def",
            "del",
            "elif",
            "else",
            "except",
            "finally",
            "for",
            "from",
            "global",
            "if",
            "import",
            "in",
            "is",
            "lambda",
            "nonlocal",
            "not",
            "or",
            "pass",
            "raise",
            "return",
            "try",
            "while",
            "with",
            "yield",
            "print",
            "len",
            "range",
            "enumerate",
            "isinstance",
        ],
    ),
    (
        "js-mode",
        &[
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "default",
            "delete",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "let",
            "new",
            "null",
            "return",
            "static",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "yield",
            "console",
        ],
    ),
    (
        "c-mode",
        &[
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
            "else", "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long",
            "register", "return", "short", "signed", "sizeof", "static", "struct", "switch",
            "typedef", "union", "unsigned", "void", "volatile", "while", "bool", "size_t",
            "uint8_t", "uint32_t", "int64_t", "NULL", "printf", "malloc", "free",
        ],
    ),
    (
        "go-mode",
        &[
            "break",
            "case",
            "chan",
            "const",
            "continue",
            "default",
            "defer",
            "else",
            "fallthrough",
            "for",
            "func",
            "go",
            "goto",
            "if",
            "import",
            "interface",
            "map",
            "package",
            "range",
            "return",
            "select",
            "struct",
            "switch",
            "type",
            "var",
            "nil",
            "true",
            "false",
            "string",
            "error",
            "make",
            "append",
            "len",
        ],
    ),
    (
        "lua-mode",
        &[
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto",
            "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until",
            "while", "pairs", "ipairs", "print", "require", "string", "table", "tostring",
            "tonumber",
        ],
    ),
    (
        "sh-mode",
        &[
            "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in",
            "local", "return", "select", "then", "until", "while", "echo", "export", "printf",
            "readonly",
        ],
    ),
];

/// Corfu's own defaults, the ones the policy below starts from.
const DEFAULT_CYCLE: bool = true;
const DEFAULT_PRESELECT_PROMPT: bool = false;
const DEFAULT_SEPARATOR: char = ' ';
const DEFAULT_QUICK1: &str = "asdfgh";
const DEFAULT_QUICK2: &str = "jkluionm";

thread_local! {
    static AUTO: Cell<bool> = const { Cell::new(true) };
    static DELAY_MS: Cell<u32> = const { Cell::new(DEFAULT_DELAY_MS) };
    static MIN_PREFIX: Cell<u32> = const { Cell::new(DEFAULT_MIN_PREFIX) };
    static LINES: Cell<u32> = const { Cell::new(DEFAULT_LINES) };
    static MIN_WIDTH: Cell<u32> = const { Cell::new(DEFAULT_MIN_WIDTH) };
    static MAX_WIDTH: Cell<u32> = const { Cell::new(DEFAULT_MAX_WIDTH) };
    static SORTING: RefCell<String> = RefCell::new(DEFAULT_SORT.to_string());
    static CYCLE: Cell<bool> = const { Cell::new(DEFAULT_CYCLE) };
    static PRESELECT_PROMPT: Cell<bool> = const { Cell::new(DEFAULT_PRESELECT_PROMPT) };
    static INDEXED: Cell<bool> = const { Cell::new(false) };
    static ECHO: Cell<bool> = const { Cell::new(false) };
    static ECHO_DELAY_MS: Cell<u32> = const { Cell::new(1000) };
    static MOUSE: Cell<bool> = const { Cell::new(false) };
    static HISTORY: Cell<bool> = const { Cell::new(false) };
    static QUICK1: RefCell<String> = RefCell::new(DEFAULT_QUICK1.to_string());
    static QUICK2: RefCell<String> = RefCell::new(DEFAULT_QUICK2.to_string());
    static SEPARATOR: Cell<char> = const { Cell::new(DEFAULT_SEPARATOR) };
    // corfu-popupinfo: the auxiliary popup beside the completion popup.
    static POPUP_INFO: Cell<bool> = const { Cell::new(false) };
    static INFO_DOCUMENTATION: Cell<bool> = const { Cell::new(true) };
    static INFO_DELAY_MS: Cell<u32> = const { Cell::new(DEFAULT_INFO_DELAY_MS) };
    static INFO_MAX_WIDTH: Cell<u32> = const { Cell::new(DEFAULT_INFO_MAX_WIDTH) };
    static INFO_MAX_HEIGHT: Cell<u32> = const { Cell::new(DEFAULT_INFO_MAX_HEIGHT) };
    // corfu-minibuffer-mode: complete in the minibuffer too.
    static MINIBUFFER: Cell<bool> = const { Cell::new(false) };
}

/// The popup policy this plugin wants (completion.set-options).
fn options() -> Options {
    Options {
        cycle: CYCLE.with(Cell::get),
        preselect_prompt: PRESELECT_PROMPT.with(Cell::get),
        separator: SEPARATOR.with(Cell::get),
        quit_at_boundary: "separator".into(),
        quit_no_match: "separator".into(),
        continue_commands: vec![
            "ignore".into(),
            "universal-argument".into(),
            "universal-argument-more".into(),
            "digit-argument".into(),
            "mwheel-scroll".into(),
            "corfu-*".into(),
            "completion-popup-*".into(),
            "scroll-other-window*".into(),
        ],
        indexed: INDEXED.with(Cell::get),
        quick1: QUICK1.with(|q| q.borrow().clone()),
        quick2: QUICK2.with(|q| q.borrow().clone()),
        echo: ECHO.with(Cell::get),
        echo_delay_ms: ECHO_DELAY_MS.with(Cell::get),
        mouse: MOUSE.with(Cell::get),
        history: HISTORY.with(Cell::get),
    }
}

/// Where the word before `col` starts (letters, digits, `_`), in chars.
pub fn word_start(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut s = col.min(chars.len());
    while s > 0 && (chars[s - 1].is_alphanumeric() || chars[s - 1] == '_') {
        s -= 1;
    }
    s
}

/// Keywords of `mode` that extend `prefix` (not equal to it).
pub fn keywords(mode: &str, prefix: &str) -> Vec<&'static str> {
    if prefix.is_empty() {
        return Vec::new();
    }
    KEYWORDS
        .iter()
        .find(|(m, _)| *m == mode)
        .map(|(_, ks)| {
            ks.iter()
                .copied()
                .filter(|k| *k != prefix && k.starts_with(prefix))
                .collect()
        })
        .unwrap_or_default()
}

fn set_ui(auto: bool) {
    let lines = LINES.with(Cell::get).clamp(1, 50);
    let delay = DELAY_MS.with(Cell::get).clamp(50, 2_000);
    let min_prefix = MIN_PREFIX.with(Cell::get).clamp(1, 32);
    completion::set_at_point_ui(true, auto, delay, min_prefix, lines);
    // The popup grows to fit its candidates, within the range corfu asks
    // for (corfu-min-width, corfu-max-width).
    set_popup_frame(MIN_WIDTH.with(Cell::get), MAX_WIDTH.with(Cell::get), 0);
    // Corfu's own default order: shortest first, then alphabetical.
    let style = SORTING.with(|s| s.borrow().clone());
    set_sorting(&[style.as_str()]);
    // How the popup selects, takes and quits (corfu-cycle, corfu-preselect,
    // the modes below, corfu-separator).
    set_options(&options());
    // The auxiliary popup beside it (corfu-popupinfo): the documentation
    // or the location of the selection, after a pause. Both off turns it
    // off while the mode is off.
    let info = POPUP_INFO.with(Cell::get);
    let doc = info && INFO_DOCUMENTATION.with(Cell::get);
    set_popup_info(
        doc,
        info && !doc,
        INFO_DELAY_MS.with(Cell::get),
        INFO_MAX_WIDTH.with(Cell::get),
        INFO_MAX_HEIGHT.with(Cell::get),
    );
}

/// Flip one of the modes and say which way it went.
fn toggle(name: &str, cell: &'static std::thread::LocalKey<Cell<bool>>) {
    let on = !cell.with(Cell::get);
    cell.with(|c| c.set(on));
    set_options(&options());
    message(&format!("{name} {}", if on { "on" } else { "off" }));
}

/// Set a `Cell<u32>` and re-apply the UI.
fn set_u32(cell: &'static std::thread::LocalKey<Cell<u32>>, n: u32) {
    cell.with(|c| c.set(n));
    set_ui(AUTO.with(Cell::get));
}

fn init() -> Result<(), String> {
    let modes: Vec<String> = KEYWORDS.iter().map(|(m, _)| m.to_string()).collect();
    completion_source("keywords", &modes, |req| {
        let Some(b) = Buffer::from_id(req.buffer) else {
            return;
        };
        let line = b.line(req.point.line);
        let col = req.point.column as usize;
        let start = word_start(&line, col);
        let prefix: String = line.chars().skip(start).take(col - start).collect();
        let items: Vec<_> = keywords(&b.major_mode(), &prefix)
            .into_iter()
            .map(|k| candidate(k, Some("keyword")))
            .collect();
        let span = Span {
            start: Position {
                line: req.point.line,
                column: start as u32,
            },
            end: req.point,
        };
        req.deliver(span, &items);
    })?;
    set_ui(true);
    command(
        "corfu-auto-mode",
        "Turn opening the completion popup by itself on or off.",
        |_| {
            let auto = !AUTO.with(Cell::get);
            AUTO.with(|a| a.set(auto));
            set_ui(auto);
            message(if auto {
                "Corfu auto completion on"
            } else {
                "Corfu auto completion off (M-TAB still completes)"
            });
            Ok(())
        },
    )?;
    command(
        "corfu-count",
        "How many rows the popup shows (corfu-count). With a numeric prefix argument, set it to that.",
        |prefix| {
            if let Some(n) = prefix {
                set_u32(&LINES, n as u32);
            }
            message(&format!("Corfu shows {} rows", LINES.with(Cell::get)));
            Ok(())
        },
    )?;
    command(
        "corfu-auto-delay",
        "How long typing must pause before the popup opens, in milliseconds (corfu-auto-delay).",
        |prefix| {
            if let Some(n) = prefix {
                set_u32(&DELAY_MS, n as u32);
            }
            message(&format!("Corfu opens after {}ms", DELAY_MS.with(Cell::get)));
            Ok(())
        },
    )?;
    command(
        "corfu-auto-prefix",
        "How many word characters must be typed before the popup opens (corfu-auto-prefix).",
        |prefix| {
            if let Some(n) = prefix {
                set_u32(&MIN_PREFIX, n as u32);
            }
            message(&format!(
                "Corfu opens after {} characters",
                MIN_PREFIX.with(Cell::get)
            ));
            Ok(())
        },
    )?;
    command(
        "corfu-sort",
        "Cycle how the popup orders its candidates (corfu-sort-function).",
        |_| {
            let next = SORTING.with(|s| {
                let cur = s.borrow().clone();
                let i = SORTS.iter().position(|n| *n == cur).unwrap_or(0);
                let next = SORTS[(i + 1) % SORTS.len()].to_string();
                *s.borrow_mut() = next.clone();
                next
            });
            set_ui(AUTO.with(Cell::get));
            message(&format!("Corfu sorts by {next}"));
            Ok(())
        },
    )?;
    command(
        "corfu-info-documentation",
        "Show the documentation of the candidate under point in the completion popup (corfu-info-documentation).",
        |_| {
            let Some(s) = selected() else {
                message("No completion popup is open");
                return Ok(());
            };
            message(&documentation_note(&s));
            Ok(())
        },
    )?;
    command(
        "corfu-info-location",
        "Say where the candidate under point in the completion popup comes from (corfu-info-location).",
        |_| {
            let Some(s) = selected() else {
                message("No completion popup is open");
                return Ok(());
            };
            // A source may have given the place up front; else ask it (the
            // answer arrives later and the source shows it itself).
            if s.location.is_some() || !locate() {
                message(&location_note(&s));
            }
            Ok(())
        },
    )?;
    // Moving, taking and scrolling: thin wrappers over the popup's own
    // controls, so the popup can move and quit without a per-key hook.
    command("corfu-next", "Select the next candidate.", |_| {
        next(1);
        Ok(())
    })?;
    command("corfu-previous", "Select the previous candidate.", |_| {
        previous(1);
        Ok(())
    })?;
    command("corfu-first", "Select the first candidate.", |_| {
        first();
        Ok(())
    })?;
    command("corfu-last", "Select the last candidate.", |_| {
        last();
        Ok(())
    })?;
    command("corfu-scroll-up", "Select the candidate a page up.", |_| {
        scroll(true);
        Ok(())
    })?;
    command(
        "corfu-scroll-down",
        "Select the candidate a page down.",
        |_| {
            scroll(false);
            Ok(())
        },
    )?;
    command(
        "corfu-complete",
        "Take the candidate under the cursor (TAB).",
        |_| {
            accept();
            Ok(())
        },
    )?;
    command(
        "corfu-insert",
        "Take the candidate under the cursor (RET).",
        |_| {
            accept();
            Ok(())
        },
    )?;
    command(
        "corfu-expand",
        "Grow the typed text to what every candidate shares (M-TAB).",
        |_| {
            expand();
            Ok(())
        },
    )?;
    command(
        "corfu-reset",
        "Deselect the candidate; again, close the popup.",
        |_| {
            reset();
            Ok(())
        },
    )?;
    command(
        "corfu-quit",
        "Close the popup, keeping what was typed.",
        |_| {
            quit();
            Ok(())
        },
    )?;
    command(
        "corfu-insert-separator",
        "Insert the component separator and keep completing.",
        |_| {
            insert_separator();
            Ok(())
        },
    )?;
    command(
        "corfu-send",
        "Take the candidate and send the line (comint-send-input, else newline).",
        |_| {
            accept();
            let send = if command_exists("comint-send-input") {
                "comint-send-input"
            } else {
                "newline"
            };
            run_command(send)?;
            Ok(())
        },
    )?;
    command(
        "corfu-quick-insert",
        "Press a key shown beside a candidate to take it (corfu-quick-insert).",
        |_| {
            arm_quick("insert");
            Ok(())
        },
    )?;
    command(
        "corfu-quick-complete",
        "Press a key shown beside a candidate to take it (corfu-quick-complete).",
        |_| {
            arm_quick("complete");
            Ok(())
        },
    )?;
    command(
        "corfu-quick-jump",
        "Press a key shown beside a candidate to select it (corfu-quick-jump).",
        |_| {
            arm_quick("jump");
            Ok(())
        },
    )?;
    command(
        "corfu-indexed-mode",
        "Number the popup's rows; C-u N TAB takes row N (corfu-indexed-mode).",
        |_| {
            toggle("Corfu indexed mode", &INDEXED);
            Ok(())
        },
    )?;
    command(
        "corfu-echo-mode",
        "Show the selected candidate's documentation in the echo area after a pause (corfu-echo-mode).",
        |_| {
            toggle("Corfu echo mode", &ECHO);
            Ok(())
        },
    )?;
    command(
        "corfu-mouse-mode",
        "Click a candidate in the popup to take it (corfu-mouse-mode).",
        |_| {
            toggle("Corfu mouse mode", &MOUSE);
            Ok(())
        },
    )?;
    command(
        "corfu-history-mode",
        "Remember the candidates you take, across sessions, and list them first (corfu-history-mode).",
        |_| {
            toggle("Corfu history mode", &HISTORY);
            Ok(())
        },
    )?;
    // corfu-minibuffer-mode (global-corfu-minibuffer): complete in the
    // minibuffer with the editor's list under the prompt, the same thing
    // vertico turns on, at corfu-count rows. Off by default, as in corfu.
    command(
        "corfu-minibuffer-mode",
        "Complete in the minibuffer too: list and filter prompt candidates the way the popup does (corfu-minibuffer-mode).",
        |_| {
            let on = !MINIBUFFER.with(Cell::get);
            MINIBUFFER.with(|c| c.set(on));
            completion::set_minibuffer_ui(on, LINES.with(Cell::get).clamp(1, 50));
            message(&format!(
                "Corfu minibuffer mode {}",
                if on { "on" } else { "off" }
            ));
            Ok(())
        },
    )?;
    // corfu-popupinfo: an auxiliary popup beside the completion popup with
    // the selected candidate's documentation or location (corfu-popupinfo).
    command(
        "corfu-popupinfo-mode",
        "Show the selected candidate's documentation (or location) in an auxiliary popup (corfu-popupinfo-mode).",
        |_| {
            let on = !POPUP_INFO.with(Cell::get);
            POPUP_INFO.with(|c| c.set(on));
            set_ui(AUTO.with(Cell::get));
            // corfu-popupinfo-map is installed only while the mode is on:
            // M-h and M-g move from the echo area to the popup.
            let (doc, loc) = if on {
                ("corfu-popupinfo-documentation", "corfu-popupinfo-location")
            } else {
                ("corfu-info-documentation", "corfu-info-location")
            };
            let _ = bind_in_prompt(POPUP, "M-h", doc);
            let _ = bind_in_prompt(POPUP, "M-g", loc);
            message(&format!(
                "Corfu popupinfo mode {}",
                if on { "on" } else { "off" }
            ));
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-toggle",
        "Toggle the auxiliary popup display or hide (corfu-popupinfo-toggle).",
        |_| {
            toggle_popup_info();
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-documentation",
        "Show or hide documentation in the auxiliary popup (corfu-popupinfo-documentation).",
        |_| {
            INFO_DOCUMENTATION.with(|c| c.set(true));
            set_ui(AUTO.with(Cell::get));
            toggle_popup_info();
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-location",
        "Show or hide location in the auxiliary popup (corfu-popupinfo-location).",
        |_| {
            INFO_DOCUMENTATION.with(|c| c.set(false));
            set_ui(AUTO.with(Cell::get));
            toggle_popup_info();
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-scroll-up",
        "Scroll the auxiliary popup a page up (corfu-popupinfo-scroll-up).",
        |_| {
            scroll_popup_info(false);
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-scroll-down",
        "Scroll the auxiliary popup a page down (corfu-popupinfo-scroll-down).",
        |_| {
            scroll_popup_info(true);
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-beginning",
        "Scroll the auxiliary popup to its beginning (corfu-popupinfo-beginning).",
        |_| {
            popup_info_beginning();
            Ok(())
        },
    )?;
    command(
        "corfu-popupinfo-end",
        "Scroll the auxiliary popup to its end (corfu-popupinfo-end).",
        |_| {
            popup_info_end();
            Ok(())
        },
    )?;
    // corfu-map: everything corfu binds in the popup's own map rather than
    // the global one, so it means something only while a popup is open.
    // M-TAB expands, and that is Forge's.
    let _ = bind_in_prompt(POPUP, "M-g", "corfu-info-location");
    let _ = bind_in_prompt(POPUP, "M-h", "corfu-info-documentation");
    let _ = bind_in_prompt(POPUP, "M-<", "corfu-first");
    let _ = bind_in_prompt(POPUP, "M->", "corfu-last");
    let _ = bind_in_prompt(POPUP, "C-v", "corfu-scroll-up");
    let _ = bind_in_prompt(POPUP, "M-v", "corfu-scroll-down");
    let _ = bind_in_prompt(POPUP, "<next>", "corfu-scroll-up");
    let _ = bind_in_prompt(POPUP, "<prior>", "corfu-scroll-down");
    let _ = bind_in_prompt(POPUP, "C-q", "corfu-quick-insert");
    let _ = bind_in_prompt(POPUP, "M-q", "corfu-quick-complete");
    let _ = bind_in_prompt(POPUP, "M-SPC", "corfu-insert-separator");
    // corfu-popupinfo-map (corfu-popupinfo-mode): the auxiliary popup's
    // toggle and scrolling. M-h and M-g are (re)bound by the mode command.
    let _ = bind_in_prompt(POPUP, "M-t", "corfu-popupinfo-toggle");
    let _ = bind_in_prompt(POPUP, "M-{", "corfu-popupinfo-beginning");
    let _ = bind_in_prompt(POPUP, "M-}", "corfu-popupinfo-end");
    let _ = bind_in_prompt(POPUP, "C-M-v", "corfu-popupinfo-scroll-up");
    let _ = bind_in_prompt(POPUP, "C-M-S-v", "corfu-popupinfo-scroll-down");
    // corfu-map in the prompt's list (corfu-minibuffer-mode): the same
    // move/scroll keys as the popup, on Forge's minibuffer list. The
    // commands only act while a list is up, so they are harmless otherwise.
    let _ = bind_in_prompt(LIST, "M-<", "minibuffer-list-first");
    let _ = bind_in_prompt(LIST, "M->", "minibuffer-list-last");
    let _ = bind_in_prompt(LIST, "C-v", "minibuffer-list-prev-page");
    let _ = bind_in_prompt(LIST, "M-v", "minibuffer-list-next-page");
    Ok(())
}

forge_plugin!(init);

/// What corfu-info-documentation says about `s`: the source's
/// documentation, else its annotation, else a "no documentation" note.
fn documentation_note(s: &Selection) -> String {
    let note = s
        .documentation
        .as_deref()
        .or(s.annotation.as_deref())
        .unwrap_or("");
    if note.is_empty() {
        format!("{} has no documentation here", s.text)
    } else {
        format!("{}: {note}", s.text)
    }
}

/// What corfu-info-location says about `s`: the source's location (1-based,
/// as Emacs counts lines) if it gave one, else the category and position.
fn location_note(s: &Selection) -> String {
    match &s.location {
        Some(l) => {
            let line = l.line + 1;
            let col = l.column + 1;
            match &l.path {
                Some(p) => {
                    let name = std::path::Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.clone());
                    format!("{} ({}:{}:{})", s.text, name, line, col)
                }
                None => format!("{} (line {}, column {})", s.text, line, col),
            }
        }
        None => format!("{} ({}, {}/{})", s.text, s.category, s.index, s.total),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_plugin_sdk::Location;

    #[test]
    fn words_and_keywords() {
        assert_eq!(word_start("    let x = mat", 15), 12);
        assert_eq!(word_start("héllo wörl", 10), 6, "columns are characters");
        assert_eq!(word_start("x.", 2), 2);
        assert_eq!(keywords("rust-mode", "ma"), ["match"]);
        assert_eq!(
            keywords("rust-mode", "match"),
            Vec::<&str>::new(),
            "not the word itself"
        );
        assert!(keywords("text-mode", "ma").is_empty());
        assert!(keywords("rust-mode", "").is_empty());
    }

    #[test]
    fn the_defaults_are_corfus_own() {
        assert_eq!(DEFAULT_DELAY_MS, 200, "corfu-auto-delay, in ms");
        assert_eq!(DEFAULT_MIN_PREFIX, 3, "corfu-auto-prefix");
        assert_eq!(DEFAULT_LINES, 10, "corfu-count");
        assert_eq!(DEFAULT_MIN_WIDTH, 15, "corfu-min-width");
        assert_eq!(DEFAULT_MAX_WIDTH, 100, "corfu-max-width");
    }

    #[test]
    fn every_sort_name_is_one_the_host_knows() {
        assert_eq!(SORTS[0], DEFAULT_SORT, "length is corfu's default");
        for name in SORTS {
            assert!(
                forge_core::completion::Sort::from_name(name).is_ok(),
                "{name} isn't a sorting style the host knows"
            );
        }
    }

    #[test]
    fn every_keyword_mode_has_a_table() {
        for (mode, ks) in KEYWORDS {
            assert!(mode.ends_with("-mode"), "{mode} isn't a mode name");
            assert!(!ks.is_empty(), "{mode} has no keywords");
            assert_eq!(
                ks.iter().collect::<std::collections::BTreeSet<_>>().len(),
                ks.len(),
                "{mode} repeats a keyword"
            );
        }
    }

    /// The candidate the info commands describe.
    fn selection() -> Selection {
        Selection {
            text: "alpha".into(),
            annotation: Some("a note".into()),
            category: "point".into(),
            index: 2,
            total: 5,
            documentation: None,
            location: None,
        }
    }

    #[test]
    fn documentation_prefers_the_source_documentation_then_the_annotation() {
        let mut s = selection();
        assert_eq!(documentation_note(&s), "alpha: a note");
        s.documentation = Some("The first letter.".into());
        assert_eq!(documentation_note(&s), "alpha: The first letter.");
        s.documentation = None;
        s.annotation = None;
        assert_eq!(documentation_note(&s), "alpha has no documentation here");
    }

    #[test]
    fn location_prefers_the_source_location_then_the_position() {
        let mut s = selection();
        assert_eq!(location_note(&s), "alpha (point, 2/5)");
        s.location = Some(Location {
            path: None,
            line: 3,
            column: 7,
        });
        assert_eq!(location_note(&s), "alpha (line 4, column 8)");
        s.location = Some(Location {
            path: Some("/src/foo.rs".into()),
            line: 3,
            column: 7,
        });
        assert_eq!(location_note(&s), "alpha (foo.rs:4:8)");
    }
}
