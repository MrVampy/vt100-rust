use vt100::{CellKind, NewProcessScreenPolicy, Parser};

#[test]
fn round_trips_both_buffers_history_modes_and_pending_wrap() {
    let mut parser = Parser::new(4, 8, 8);
    parser.process(b"one\r\ntwo\r\nthree\r\nfour\r\nfive");
    parser.process("\r\nwide \u{754c}\u{0301}".as_bytes());
    parser.process(b"\x1b[1;34m\x1b7\x1b[?1h\x1b[?2004h");
    parser.process(b"\x1b[?1049h\x1b[?25lalternate");
    parser.process(b"\x1b[4;8H!");

    let state = parser.screen().state();
    state.validate().unwrap();
    assert!(state.primary_grid.scrollback.len() >= 2);
    assert!(state
        .primary_grid
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .any(|cell| cell.kind == CellKind::Wide));

    let restored = Parser::from_screen_state(state.clone()).unwrap();
    assert_eq!(restored.screen().state(), state);
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
    parser.process(b"\x1b[1;34m\x1b7\x1b[?1h\x1b[?2004h");
    parser.process(b"\x1b[?1049h\x1b[?25lalternate\x1b[31");
    let before = parser.screen().state();
    assert!(!before.primary_grid.scrollback.is_empty());

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
