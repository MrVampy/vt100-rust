use vt100::{CellKind, NewProcessScreenPolicy, Parser};

#[test]
fn round_trips_both_buffers_history_modes_and_pending_wrap() {
    let mut parser = Parser::new(4, 8, 8);
    parser.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
    parser.process("\r\nwide \u{754c}\u{0301}".as_bytes());
    parser.process(b"\x1b[1;34m\x1b7\x1b[?1h\x1b[?2004h\x1b[5 q");
    parser.screen_mut().enter_retained_alternate_screen();
    parser.process(b"\x1b[?25lalternate");
    parser.process(b"\r\none\r\ntwo\r\nthree\r\nfour\r\nfive");
    parser.process(b"\x1b[4;8H!");

    let state = parser.screen().state();
    state.validate().unwrap();
    assert!(state.primary_grid.scrollback.len() >= 2);
    assert!(!state.alternate_grid.scrollback.is_empty());
    assert!(state
        .primary_grid
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .any(|cell| cell.kind == CellKind::Wide));
    assert_eq!(state.modes.cursor_style, vt100::CursorStyle::BlinkingBar);

    let restored = Parser::from_screen_state(state.clone()).unwrap();
    assert_eq!(restored.screen().state(), state);
}

#[test]
fn retained_alternate_history_is_bounded_and_each_lifetime_resets_it() {
    let mut parser = Parser::new(3, 12, 4);
    parser.process(b"shell");
    parser.screen_mut().enter_retained_alternate_screen();
    for line in 0..8 {
        parser.process(format!("alternate-{line}\r\n").as_bytes());
    }

    let first = parser.screen().state();
    assert_eq!(first.alternate_grid.scrollback.len(), 4);
    assert!(first.alternate_grid.scrollback_top > 0);
    assert!(first.primary_grid.scrollback.is_empty());

    parser.screen_mut().exit_retained_alternate_screen();
    assert_eq!(parser.screen().contents(), "shell");
    parser.screen_mut().enter_retained_alternate_screen();
    parser.process(b"new");

    let second = parser.screen().state();
    assert!(second.alternate_grid.scrollback.is_empty());
    assert_eq!(second.alternate_grid.scrollback_top, 0);
    assert!(parser.screen().contents().starts_with("new"));
}

#[test]
fn retained_alternate_screen_with_no_history_still_restores_the_primary_screen(
) {
    let mut parser = Parser::new(3, 12, 0);
    parser.process(b"shell");
    parser.screen_mut().enter_retained_alternate_screen();
    parser.process(b"agent");
    parser.screen_mut().exit_retained_alternate_screen();

    assert!(!parser.screen().alternate_screen());
    assert_eq!(parser.screen().contents(), "shell");
}

#[test]
fn standard_1049_alternate_screen_remains_history_free() {
    let mut parser = Parser::new(3, 12, 4);
    parser.process(b"shell\x1b[?1049h");
    for line in 0..8 {
        parser.process(format!("alternate-{line}\r\n").as_bytes());
    }

    let alternate = parser.screen().state();
    assert_eq!(alternate.alternate_grid.scrollback_limit, 0);
    assert!(alternate.alternate_grid.scrollback.is_empty());
    assert!(alternate.alternate_grid.scrollback_top > 0);

    parser.process(b"\x1b[?1049l");
    assert_eq!(parser.screen().contents(), "shell");
}

#[test]
fn restored_parser_does_not_complete_the_previous_parsers_escape_sequence() {
    let mut parser = Parser::new(4, 8, 0);
    parser.process(b"safe\x1b[31");
    let state = parser.screen().state();

    let mut restored = Parser::from_screen_state(state).unwrap();
    restored.process(b"mred");
    assert!(restored.screen().contents().contains("safemred"));
    assert_eq!(
        restored.screen().cell(0, 4).unwrap().fgcolor(),
        vt100::Color::Default
    );
}

#[test]
fn rejects_a_wide_cell_without_its_continuation() {
    let parser = Parser::new(4, 8, 0);
    let mut state = parser.screen().state();
    state.primary_grid.rows[0].cells[0].contents = "\u{754c}".to_string();
    state.primary_grid.rows[0].cells[0].kind = CellKind::Wide;
    assert!(Parser::from_screen_state(state).is_err());
}

#[test]
fn accepts_a_saved_origin_cursor_outside_a_replaced_scroll_region() {
    let mut parser = Parser::new(4, 8, 0);
    parser.process(b"\x1b[1;2r\x1b[?6h\x1b[1;1H\x1b7\x1b[3;4r\x1b8");
    let state = parser.screen().state();
    state.validate().unwrap();
    assert!(state.primary_grid.origin_mode);
    assert!(state.primary_grid.cursor.row < state.primary_grid.scroll_top);
}

#[test]
fn replacement_process_reset_preserves_scrollback_and_clears_live_state() {
    let mut parser = Parser::new(4, 8, 8);
    parser.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix");
    parser.process(b"\x1b[1;34m\x1b7\x1b[?1h\x1b[?2004h\x1b[6 q");
    parser.screen_mut().enter_retained_alternate_screen();
    parser.process(b"\x1b[?25lalternate\x1b[31");
    let before = parser.screen().state();
    assert!(!before.primary_grid.scrollback.is_empty());
    assert_eq!(
        before.alternate_grid.scrollback_limit,
        before.primary_grid.scrollback_limit
    );

    parser.reset_for_new_process(NewProcessScreenPolicy::DiscardLiveScreen);
    let reset = parser.screen().state();

    assert_eq!(
        reset.primary_grid.scrollback,
        before.primary_grid.scrollback
    );
    assert_eq!(
        reset.primary_grid.scrollback_top,
        before.primary_grid.scrollback_top
    );
    assert_eq!(
        reset.primary_grid.scrollback_limit,
        before.primary_grid.scrollback_limit
    );
    assert!(reset.alternate_grid.scrollback.is_empty());
    assert_eq!(reset.alternate_grid.scrollback_top, 0);
    assert_eq!(reset.alternate_grid.scrollback_limit, 0);
    assert!(reset
        .primary_grid
        .rows
        .iter()
        .chain(&reset.alternate_grid.rows)
        .flat_map(|row| &row.cells)
        .all(|cell| cell.contents.is_empty()));
    assert_eq!(reset.primary_grid.cursor, vt100::Position::default());
    assert_eq!(reset.primary_grid.saved_cursor, vt100::Position::default());
    assert_eq!(reset.primary_grid.scroll_top, 0);
    assert_eq!(reset.primary_grid.scroll_bottom, 3);
    assert_eq!(reset.attributes, vt100::CellAttributes::default());
    assert_eq!(reset.saved_attributes, vt100::CellAttributes::default());
    assert_eq!(reset.modes, vt100::ScreenModes::default());

    parser.process(b"mnew");
    let live = parser.screen().state();
    assert_eq!(live.primary_grid.rows[0].cells[0].contents, "m");
    assert_eq!(
        live.primary_grid.rows[0].cells[0].attributes.foreground,
        vt100::Color::Default
    );
}

#[test]
fn replacement_process_can_preserve_meaningful_live_rows_as_scrollback() {
    let mut parser = Parser::new(5, 12, 8);
    parser.process(b"one\r\n\r\ntwo");
    let before = parser.screen().state();
    assert!(before.primary_grid.scrollback.is_empty());

    parser.reset_for_new_process(
        NewProcessScreenPolicy::PreserveLiveScreenAsScrollback,
    );
    let reset = parser.screen().state();

    assert_eq!(reset.primary_grid.scrollback.len(), 3);
    assert_eq!(reset.primary_grid.scrollback[0].cells[0].contents, "o");
    assert!(reset.primary_grid.scrollback[1]
        .cells
        .iter()
        .all(|cell| cell.contents.is_empty()));
    assert_eq!(reset.primary_grid.scrollback[2].cells[0].contents, "t");
    assert!(!reset.primary_grid.scrollback[2].wrapped);
    assert!(reset
        .primary_grid
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .all(|cell| cell.contents.is_empty()));

    parser.process(b"new");
    let live = parser.screen().state();
    assert_eq!(live.primary_grid.rows[0].cells[0].contents, "n");
    assert_eq!(live.primary_grid.scrollback, reset.primary_grid.scrollback);
}
