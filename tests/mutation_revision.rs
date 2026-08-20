#[derive(Default)]
struct Diagnostics {
    titles: usize,
    unsupported: usize,
}

impl vt100::Callbacks for Diagnostics {
    fn set_window_title(&mut self, _: &mut vt100::Screen, _: &[u8]) {
        self.titles += 1;
    }

    fn set_window_icon_name(&mut self, _: &mut vt100::Screen, _: &[u8]) {
        self.titles += 1;
    }

    fn unhandled_csi(
        &mut self,
        _: &mut vt100::Screen,
        _: Option<u8>,
        _: Option<u8>,
        _: &[&[u16]],
        _: char,
    ) {
        self.unsupported += 1;
    }

    fn unhandled_osc(&mut self, _: &mut vt100::Screen, _: &[&[u8]]) {
        self.unsupported += 1;
    }
}

#[test]
fn diagnostic_only_sequences_do_not_advance_screen_mutation_revision() {
    let mut parser =
        vt100::Parser::new_with_callbacks(24, 80, 0, Diagnostics::default());
    let initial = parser.screen_mutation_revision();

    parser.process(
        b"\x07\x1b]0;working\x07\x1b]777;ignored\x07\x1b[?2026h\x1b[?2026l",
    );

    assert_eq!(parser.screen_mutation_revision(), initial);
    assert_eq!(parser.callbacks().titles, 2);
    assert_eq!(parser.callbacks().unsupported, 3);
}

#[test]
fn parser_owned_screen_actions_advance_the_mutation_revision() {
    let mut parser = vt100::Parser::default();
    let initial = parser.screen_mutation_revision();

    parser.process(b"x");
    let after_text = parser.screen_mutation_revision();
    assert!(after_text > initial);

    parser.process(b"\x1b[31m");
    let after_attributes = parser.screen_mutation_revision();
    assert!(after_attributes > after_text);

    parser.process(b"\x1b[H");
    let after_cursor = parser.screen_mutation_revision();
    assert!(after_cursor > after_attributes);

    parser.reset_for_new_process(
        vt100::NewProcessScreenPolicy::DiscardLiveScreen,
    );
    assert!(parser.screen_mutation_revision() > after_cursor);
}
