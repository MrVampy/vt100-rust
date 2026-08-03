use unicode_width::UnicodeWidthChar as _;

/// A validated, parser-independent copy of the terminal's visible state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenState {
    /// The number of terminal rows.
    pub rows: u16,
    /// The number of terminal columns.
    pub columns: u16,
    /// The primary screen grid and retained history.
    pub primary_grid: GridState,
    /// The alternate screen grid.
    pub alternate_grid: GridState,
    /// Attributes used for newly printed cells.
    pub attributes: CellAttributes,
    /// Attributes restored with the saved cursor.
    pub saved_attributes: CellAttributes,
    /// Safe input and presentation modes.
    pub modes: ScreenModes,
}

impl ScreenState {
    /// Verifies that this state can be imported without violating emulator invariants.
    pub fn validate(&self) -> Result<(), ScreenStateError> {
        if self.rows == 0 || self.columns == 0 {
            return Err(ScreenStateError::new("screen dimensions are empty"));
        }
        validate_attributes(&self.attributes)?;
        validate_attributes(&self.saved_attributes)?;
        validate_grid(self.rows, self.columns, &self.primary_grid)?;
        validate_grid(self.rows, self.columns, &self.alternate_grid)?;
        if self.alternate_grid.scrollback_limit != 0
            || !self.alternate_grid.scrollback.is_empty()
        {
            return Err(ScreenStateError::new(
                "alternate grid cannot retain scrollback",
            ));
        }
        Ok(())
    }
}

/// A terminal grid, including its cursor and retained rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridState {
    /// The current cursor position.
    pub cursor: Position,
    /// The saved cursor position.
    pub saved_cursor: Position,
    /// The first row in the scrolling region.
    pub scroll_top: u16,
    /// The last row in the scrolling region.
    pub scroll_bottom: u16,
    /// Whether cursor addressing is relative to the scrolling region.
    pub origin_mode: bool,
    /// The origin mode restored with the saved cursor.
    pub saved_origin_mode: bool,
    /// The live grid rows.
    pub rows: Vec<RowState>,
    /// The retained rows, oldest first.
    pub scrollback: Vec<RowState>,
    /// The maximum retained row count.
    pub scrollback_limit: usize,
    /// The stable index of the oldest retained row.
    pub scrollback_top: usize,
}

/// A zero-based terminal position.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Position {
    /// The zero-based row.
    pub row: u16,
    /// The zero-based column, which may equal the width for pending wrap.
    pub column: u16,
}

/// One complete terminal row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowState {
    /// The cells in column order.
    pub cells: Vec<CellState>,
    /// Whether this row wraps into the next row.
    pub wrapped: bool,
}

impl RowState {
    pub(crate) fn blank(columns: u16) -> Self {
        Self {
            cells: vec![CellState::default(); usize::from(columns)],
            wrapped: false,
        }
    }
}

/// One terminal cell and its rendering attributes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CellState {
    /// The printable character followed by any combining characters.
    pub contents: String,
    /// Whether this is a narrow cell, a wide cell, or its continuation.
    pub kind: CellKind,
    /// The cell's rendering attributes.
    pub attributes: CellAttributes,
}

/// The layout role of a cell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CellKind {
    /// A normal one-column cell, which may be empty.
    #[default]
    Narrow,
    /// The first cell of a two-column character.
    Wide,
    /// The second cell of a two-column character.
    WideContinuation,
}

/// Rendering attributes shared by cells and the active drawing pen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CellAttributes {
    /// The foreground color.
    pub foreground: crate::Color,
    /// The background color.
    pub background: crate::Color,
    /// Whether bold intensity is enabled.
    pub bold: bool,
    /// Whether dim intensity is enabled.
    pub dim: bool,
    /// Whether italic text is enabled.
    pub italic: bool,
    /// Whether underlining is enabled.
    pub underline: bool,
    /// Whether foreground and background are inverted.
    pub inverse: bool,
}

/// The active screen buffer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ActiveBuffer {
    /// The primary screen buffer.
    #[default]
    Primary,
    /// The alternate screen buffer.
    Alternate,
}

/// Input and presentation modes represented by the emulator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenModes {
    /// The active screen buffer.
    pub active_buffer: ActiveBuffer,
    /// Whether the cursor is visible.
    pub cursor_visible: bool,
    /// Whether application keypad mode is enabled.
    pub application_keypad: bool,
    /// Whether application cursor mode is enabled.
    pub application_cursor: bool,
    /// Whether bracketed paste mode is enabled.
    pub bracketed_paste: bool,
    /// The active mouse reporting mode.
    pub mouse_protocol_mode: crate::MouseProtocolMode,
    /// The active mouse reporting encoding.
    pub mouse_protocol_encoding: crate::MouseProtocolEncoding,
}

impl Default for ScreenModes {
    fn default() -> Self {
        Self {
            active_buffer: ActiveBuffer::Primary,
            cursor_visible: true,
            application_keypad: false,
            application_cursor: false,
            bracketed_paste: false,
            mouse_protocol_mode: crate::MouseProtocolMode::default(),
            mouse_protocol_encoding: crate::MouseProtocolEncoding::default(),
        }
    }
}

/// An invalid visual state supplied for parser construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScreenStateError {
    reason: &'static str,
}

impl ScreenStateError {
    pub(crate) const fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    /// Returns a stable description without embedding terminal contents.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl std::fmt::Display for ScreenStateError {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.write_str(self.reason)
    }
}

impl std::error::Error for ScreenStateError {}

fn validate_grid(
    rows: u16,
    columns: u16,
    grid: &GridState,
) -> Result<(), ScreenStateError> {
    validate_position(rows, columns, grid.cursor)?;
    validate_position(rows, columns, grid.saved_cursor)?;
    if grid.scroll_top > grid.scroll_bottom || grid.scroll_bottom >= rows {
        return Err(ScreenStateError::new("scroll region is invalid"));
    }
    if grid.rows.len() != usize::from(rows)
        || grid.scrollback.len() > grid.scrollback_limit
    {
        return Err(ScreenStateError::new("grid row count is invalid"));
    }
    grid.scrollback_top
        .checked_add(grid.scrollback.len())
        .ok_or_else(|| {
            ScreenStateError::new("scrollback coordinates overflow")
        })?;
    for row in grid.rows.iter().chain(&grid.scrollback) {
        validate_row(columns, row)?;
    }
    Ok(())
}

fn validate_position(
    rows: u16,
    columns: u16,
    position: Position,
) -> Result<(), ScreenStateError> {
    if position.row >= rows || position.column > columns {
        Err(ScreenStateError::new("cursor position is invalid"))
    } else {
        Ok(())
    }
}

fn validate_row(
    columns: u16,
    row: &RowState,
) -> Result<(), ScreenStateError> {
    if row.cells.len() != usize::from(columns) {
        return Err(ScreenStateError::new("row width is invalid"));
    }
    for (index, cell) in row.cells.iter().enumerate() {
        validate_cell(cell)?;
        match cell.kind {
            CellKind::Wide => {
                if row.cells.get(index + 1).map(|cell| cell.kind)
                    != Some(CellKind::WideContinuation)
                {
                    return Err(ScreenStateError::new(
                        "wide cell has no continuation",
                    ));
                }
            }
            CellKind::WideContinuation => {
                if index == 0 || row.cells[index - 1].kind != CellKind::Wide {
                    return Err(ScreenStateError::new(
                        "wide continuation has no leading cell",
                    ));
                }
            }
            CellKind::Narrow => {}
        }
    }
    Ok(())
}

fn validate_cell(cell: &CellState) -> Result<(), ScreenStateError> {
    if cell.contents.len() > crate::cell::CONTENT_BYTES
        || cell.contents.chars().any(|character| {
            character.is_control() || character == '\u{fffd}'
        })
    {
        return Err(ScreenStateError::new("cell text is invalid"));
    }
    validate_attributes(&cell.attributes)?;
    if cell.kind == CellKind::WideContinuation {
        return if cell.contents.is_empty()
            && cell.attributes == CellAttributes::default()
        {
            Ok(())
        } else {
            Err(ScreenStateError::new("wide continuation contains state"))
        };
    }
    let mut characters = cell.contents.chars();
    let Some(first) = characters.next() else {
        return if cell.kind == CellKind::Narrow {
            Ok(())
        } else {
            Err(ScreenStateError::new("wide cell is empty"))
        };
    };
    let expected_width = match cell.kind {
        CellKind::Narrow => 1,
        CellKind::Wide => 2,
        CellKind::WideContinuation => unreachable!(),
    };
    if first.width().unwrap_or(1) != expected_width
        || characters.any(|character| character.width() != Some(0))
    {
        return Err(ScreenStateError::new("cell width is invalid"));
    }
    Ok(())
}

fn validate_attributes(
    attributes: &CellAttributes,
) -> Result<(), ScreenStateError> {
    if attributes.bold && attributes.dim {
        Err(ScreenStateError::new("cell intensity is invalid"))
    } else {
        Ok(())
    }
}
