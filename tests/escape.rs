mod helpers;

#[test]
fn deckpam() {
    helpers::fixture("deckpam");
}

#[test]
fn ri() {
    helpers::fixture("ri");
}

#[test]
fn ris() {
    helpers::fixture("ris");
}

#[test]
fn vb() {
    struct State {
        vb: usize,
    }

    impl vt100::Callbacks for State {
        fn visual_bell(&mut self, _: &mut vt100::Screen) {
            self.vb += 1;
        }
    }

    let mut parser =
        vt100::Parser::new_with_callbacks(24, 80, 0, State { vb: 0 });
    assert_eq!(parser.callbacks().vb, 0);

    let screen = parser.screen().clone();
    parser.process(b"\x1bg");
    assert_eq!(parser.callbacks().vb, 1);
    assert_eq!(parser.screen().contents_diff(&screen), b"");

    let screen = parser.screen().clone();
    parser.process(b"\x1bg");
    assert_eq!(parser.callbacks().vb, 2);
    assert_eq!(parser.screen().contents_diff(&screen), b"");

    let screen = parser.screen().clone();
    parser.process(b"\x1bg\x1bg\x1bg");
    assert_eq!(parser.callbacks().vb, 5);
    assert_eq!(parser.screen().contents_diff(&screen), b"");

    let screen = parser.screen().clone();
    parser.process(b"foo");
    assert_eq!(parser.callbacks().vb, 5);
    assert_eq!(parser.screen().contents_diff(&screen), b"foo");

    let screen = parser.screen().clone();
    parser.process(b"ba\x1bgr");
    assert_eq!(parser.callbacks().vb, 6);
    assert_eq!(parser.screen().contents_diff(&screen), b"bar");
}

#[test]
fn decscusr() {
    struct State {
        unsupported: usize,
    }

    impl vt100::Callbacks for State {
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
    }

    let mut parser = vt100::Parser::new_with_callbacks(
        24,
        80,
        0,
        State { unsupported: 0 },
    );
    for (sequence, expected) in [
        (b"\x1b[ q".as_slice(), vt100::CursorStyle::Default),
        (b"\x1b[0 q".as_slice(), vt100::CursorStyle::Default),
        (b"\x1b[1 q".as_slice(), vt100::CursorStyle::BlinkingBlock),
        (b"\x1b[2 q".as_slice(), vt100::CursorStyle::SteadyBlock),
        (
            b"\x1b[3 q".as_slice(),
            vt100::CursorStyle::BlinkingUnderline,
        ),
        (b"\x1b[4 q".as_slice(), vt100::CursorStyle::SteadyUnderline),
        (b"\x1b[5 q".as_slice(), vt100::CursorStyle::BlinkingBar),
        (b"\x1b[6 q".as_slice(), vt100::CursorStyle::SteadyBar),
    ] {
        parser.process(sequence);
        assert_eq!(parser.screen().cursor_style(), expected);
    }

    parser.process(b"\x1b[7 q");
    assert_eq!(
        parser.screen().cursor_style(),
        vt100::CursorStyle::SteadyBar
    );
    assert_eq!(parser.callbacks().unsupported, 1);
}

#[test]
fn decsc() {
    helpers::fixture("decsc");
}

#[test]
fn decsc_resize() {
    let mut parser = vt100::Parser::new(24, 80, 0);
    parser.process(b"foo\x1b[20;70Hbar\x1b7");
    assert_eq!(parser.screen().contents(), "foo\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n                                                                     bar");
    assert_eq!(parser.screen().cursor_position(), (19, 72));
    parser.process(b"\x1b[H");
    assert_eq!(parser.screen().cursor_position(), (0, 0));
    parser.screen_mut().set_size(15, 60);
    assert_eq!(parser.screen().contents(), "foo");
    assert_eq!(parser.screen().cursor_position(), (0, 0));
    parser.process(b"y\x1b8z");
    assert_eq!(parser.screen().contents(), "yoo\n\n\n\n\n\n\n\n\n\n\n\n\n\n                                                           z");
    assert_eq!(parser.screen().cursor_position(), (14, 60));
}
