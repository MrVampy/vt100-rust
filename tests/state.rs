use vt100::{CellKind, Parser};

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
