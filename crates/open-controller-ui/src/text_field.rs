//! A text box, on one line or on several that wrap and scroll. Adapted from GPUI's `input`
//! example: the platform's input handler brings typed text and IME composition, and actions
//! bound under the `TextField` key context bring everything else, so they never clash with the
//! window's own keys.

use crate::theme::{FONT, Theme, radius, text};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, InspectorElementId, InteractiveElement, IntoElement,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Render,
    ScrollWheelEvent, SharedString, Style, Styled, Task, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WindowAppearance,
    WrappedLine, actions, div, fill, point, px, relative, size,
};
use std::ops::Range;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

/// The key context the field's bindings live under.
pub const CONTEXT: &str = "TextField";

const CARET_W: Pixels = px(1.5);
/// The Windows default.
const BLINK: Duration = Duration::from_millis(530);

actions!(
    text_field,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Top,
        Bottom,
        SelectTop,
        SelectBottom,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Enter,
        Submit,
    ]
);

/// The keys every text field answers to, as Windows text boxes do. Bound once, at startup.
pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, c),
        KeyBinding::new("ctrl-delete", DeleteWordRight, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("ctrl-left", WordLeft, c),
        KeyBinding::new("ctrl-right", WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("ctrl-home", Top, c),
        KeyBinding::new("ctrl-end", Bottom, c),
        KeyBinding::new("ctrl-shift-home", SelectTop, c),
        KeyBinding::new("ctrl-shift-end", SelectBottom, c),
        KeyBinding::new("ctrl-a", SelectAll, c),
        KeyBinding::new("ctrl-c", Copy, c),
        KeyBinding::new("ctrl-insert", Copy, c),
        KeyBinding::new("ctrl-x", Cut, c),
        KeyBinding::new("shift-delete", Cut, c),
        KeyBinding::new("ctrl-v", Paste, c),
        KeyBinding::new("shift-insert", Paste, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("shift-enter", Enter, c),
        KeyBinding::new("ctrl-enter", Submit, c),
    ]);
}

pub enum TextFieldEvent {
    /// The person changed the text (not `set_text` or `clear`).
    Changed,
    /// Enter on a one-line field, or Ctrl+Enter on either kind.
    Submit,
}

pub struct TextField {
    focus: FocusHandle,
    buf: Buffer,
    placeholder: SharedString,
    /// How many lines a multi-line field shows: it starts at the first and grows to the second.
    lines: (usize, usize),
    theme: Option<(WindowAppearance, Theme)>,
    layout: Option<Layout>,
    scroll: Point<Pixels>,
    /// Set when the caret moves, so the next frame scrolls it into view; the wheel leaves it.
    reveal: bool,
    /// The caret sits at the start of a wrapped row rather than at the end of the one before,
    /// which are the same offset in the text.
    downstream: bool,
    /// Where going up and down aims, kept across shorter rows.
    goal_x: Option<Pixels>,
    selecting: bool,
    caret_on: bool,
    blinking: bool,
    /// Drawn since the last blink, so a field taken off the screen while focused stops blinking.
    drawn: bool,
    blink: Task<()>,
}

impl EventEmitter<TextFieldEvent> for TextField {}

impl TextField {
    /// Created inside `cx.new`. `max_chars` counts characters, not bytes.
    pub fn new(cx: &mut Context<Self>, multiline: bool, max_chars: usize) -> TextField {
        TextField {
            focus: cx.focus_handle(),
            buf: Buffer { multiline, max_chars, ..Default::default() },
            placeholder: SharedString::default(),
            lines: (3, 6),
            theme: None,
            layout: None,
            scroll: Point::default(),
            reveal: false,
            downstream: false,
            goal_x: None,
            selecting: false,
            caret_on: true,
            blinking: false,
            drawn: false,
            blink: Task::ready(()),
        }
    }

    /// Shown in the text's colour for hints while the field is empty.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> TextField {
        self.placeholder = text.into();
        self
    }

    #[allow(dead_code)]
    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = text.into();
        cx.notify();
    }

    /// A multi-line field's height, in lines: `min` when short, growing to `max`, then scrolling.
    #[allow(dead_code)]
    pub fn lines(mut self, min: usize, max: usize) -> TextField {
        let min = min.max(1);
        self.lines = (min, max.max(min));
        self
    }

    pub fn text(&self) -> String {
        self.buf.text.clone()
    }

    #[allow(dead_code)]
    pub fn char_count(&self) -> usize {
        self.buf.text.chars().count()
    }

    /// Replaces the text, cleaned and cut to the limit as if pasted, with the caret at the end.
    #[allow(dead_code)]
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let len = self.buf.text.len();
        self.buf.replace(0..len, text);
        self.scroll = Point::default();
        self.moved(cx);
    }

    #[allow(dead_code)]
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
    }

    fn theme(&mut self, window: &Window) -> Theme {
        let appearance = window.appearance();
        match self.theme {
            Some((a, t)) if a == appearance => t,
            _ => {
                let t = window_theme(window);
                self.theme = Some((appearance, t));
                t
            }
        }
    }

    fn moved(&mut self, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.downstream = false;
        self.reveal = true;
        self.wake(cx);
        cx.notify();
    }

    fn put(&mut self, range: Option<Range<usize>>, new: &str, cx: &mut Context<Self>) {
        let range = range.unwrap_or_else(|| self.buf.selected.clone());
        if range.is_empty() && new.is_empty() {
            return;
        }
        self.buf.replace(range, new);
        self.moved(cx);
        cx.emit(TextFieldEvent::Changed);
    }

    fn go(&mut self, to: usize, select: bool, cx: &mut Context<Self>) {
        let to = self.buf.snap(to);
        if select {
            self.buf.select_to(to)
        } else {
            self.buf.move_to(to)
        }
        self.moved(cx);
    }

    /// Deletes the selection, or from the caret to `to` when nothing is selected.
    fn erase_to(&mut self, to: usize, cx: &mut Context<Self>) {
        let range = if self.buf.selected.is_empty() {
            let at = self.buf.cursor();
            at.min(to)..at.max(to)
        } else {
            self.buf.selected.clone()
        };
        self.put(Some(range), "", cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let b = &self.buf;
        let to = if b.selected.is_empty() { b.prev_boundary(b.cursor()) } else { b.selected.start };
        self.go(to, false, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let b = &self.buf;
        let to = if b.selected.is_empty() { b.next_boundary(b.cursor()) } else { b.selected.end };
        self.go(to, false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.prev_boundary(self.buf.cursor()), true, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.next_boundary(self.buf.cursor()), true, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.prev_word(self.buf.cursor()), false, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.next_word(self.buf.cursor()), false, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.prev_word(self.buf.cursor()), true, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.next_word(self.buf.cursor()), true, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., true, cx);
    }

    /// One row up or down, keeping to the column the caret started from. A one-line field goes
    /// to its start or end instead.
    fn vertical(&mut self, dir: f32, select: bool, cx: &mut Context<Self>) {
        let len = self.buf.text.len();
        let target = match &self.layout {
            Some(l) if self.buf.multiline && len > 0 => {
                let p = l.position(self.buf.cursor(), self.downstream);
                let x = self.goal_x.unwrap_or(p.x);
                let y = p.y + l.line_height * (0.5 + dir);
                let (to, downstream) = if y < px(0.) {
                    (0, false)
                } else if y >= l.height() {
                    (len, false)
                } else {
                    l.offset_at(point(x, y))
                };
                Some((to, downstream, x))
            }
            _ => None,
        };
        let Some((to, downstream, x)) = target else {
            return self.go(if dir < 0. { 0 } else { len }, select, cx);
        };
        self.go(to, select, cx);
        self.downstream = downstream;
        self.goal_x = Some(x);
    }

    /// To the start or end of the row on screen, as Windows text boxes do.
    fn row_edge(&mut self, end: bool, select: bool, cx: &mut Context<Self>) {
        let len = self.buf.text.len();
        let edge = match &self.layout {
            Some(l) if self.buf.multiline && len > 0 => {
                let i = l.row_index(self.buf.cursor(), self.downstream);
                let row = &l.rows[i];
                let wrapped = i > 0 && l.rows[i - 1].line == row.line;
                if end { (row.end, false) } else { (row.start, wrapped) }
            }
            _ => (if end { len } else { 0 }, false),
        };
        self.go(edge.0, select, cx);
        self.downstream = edge.1;
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.row_edge(false, false, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.row_edge(true, false, cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.row_edge(false, true, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.row_edge(true, true, cx);
    }

    fn top(&mut self, _: &Top, _: &mut Window, cx: &mut Context<Self>) {
        self.go(0, false, cx);
    }

    fn bottom(&mut self, _: &Bottom, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.text.len(), false, cx);
    }

    fn select_top(&mut self, _: &SelectTop, _: &mut Window, cx: &mut Context<Self>) {
        self.go(0, true, cx);
    }

    fn select_bottom(&mut self, _: &SelectBottom, _: &mut Window, cx: &mut Context<Self>) {
        self.go(self.buf.text.len(), true, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.buf.select_all();
        self.moved(cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.erase_to(self.buf.prev_boundary(self.buf.cursor()), cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.erase_to(self.buf.next_boundary(self.buf.cursor()), cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.erase_to(self.buf.prev_word(self.buf.cursor()), cx);
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.erase_to(self.buf.next_word(self.buf.cursor()), cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.buf.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.buf.text[self.buf.selected.clone()].to_string()));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.buf.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.buf.text[self.buf.selected.clone()].to_string()));
            self.put(None, "", cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.put(None, &text, cx);
        }
    }

    fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.buf.multiline {
            self.put(None, "\n", cx);
        } else {
            cx.emit(TextFieldEvent::Submit);
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Submit);
    }

    /// The offset under a point in the window, and whether it is the start of a wrapped row.
    fn offset_for_mouse(&self, position: Point<Pixels>) -> (usize, bool) {
        match &self.layout {
            Some(l) if !self.buf.text.is_empty() => {
                let (to, downstream) = l.offset_at(position - l.bounds.origin + l.scroll);
                (self.buf.snap(to), downstream)
            }
            _ => (0, false),
        }
    }

    fn mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let (to, downstream) = self.offset_for_mouse(ev.position);
        match ev.click_count {
            2 => {
                let word = self.buf.word_at(to);
                self.buf.move_to(word.start);
                self.buf.select_to(word.end);
            }
            n if n >= 3 => self.buf.select_all(),
            _ if ev.modifiers.shift => self.buf.select_to(to),
            _ => self.buf.move_to(to),
        }
        self.selecting = true;
        self.moved(cx);
        self.downstream = downstream;
    }

    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            let (to, downstream) = self.offset_for_mouse(ev.position);
            self.buf.select_to(to);
            self.moved(cx);
            self.downstream = downstream;
        }
    }

    fn scroll_wheel(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(l) = &self.layout else { return };
        if !self.buf.multiline || l.height() <= l.bounds.size.height {
            return;
        }
        self.scroll.y -= ev.delta.pixel_delta(l.line_height).y;
        cx.stop_propagation();
        cx.notify();
    }

    /// Starts blinking when focused and stops when not; called on every frame.
    fn blink(&mut self, focused: bool, cx: &mut Context<Self>) {
        if focused == self.blinking {
            return;
        }
        self.blinking = focused;
        self.caret_on = true;
        self.blink = if focused { blink_task(cx) } else { Task::ready(()) };
    }

    /// Shows the caret right away after a key or click, and restarts its rhythm.
    fn wake(&mut self, cx: &mut Context<Self>) {
        self.caret_on = true;
        if self.blinking {
            self.blink = blink_task(cx);
        }
    }
}

fn blink_task(cx: &mut Context<TextField>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(BLINK).await;
            let going = this.update(cx, |f, cx| {
                if !std::mem::take(&mut f.drawn) {
                    f.blinking = false;
                    return false;
                }
                f.caret_on = !f.caret_on;
                cx.notify();
                true
            });
            if !matches!(going, Ok(true)) {
                break;
            }
        }
    })
}

/// The window's theme, picked as `ui` picks it (`OPEN_CONTROLLER_THEME` included).
fn window_theme(window: &Window) -> Theme {
    Theme::new(match std::env::var("OPEN_CONTROLLER_THEME").as_deref() {
        Ok("light") => WindowAppearance::Light,
        Ok("dark") => WindowAppearance::Dark,
        _ => window.appearance(),
    })
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme(window);
        let focused = self.focus.is_focused(window);
        self.drawn = true;
        self.blink(focused, cx);
        let colors = Colors { text: t.text, placeholder: t.text3, selection: t.accent.opacity(0.3), caret: t.text };
        let hover = t.control_hover;
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::top))
            .on_action(cx.listener(Self::bottom))
            .on_action(cx.listener(Self::select_top))
            .on_action(cx.listener(Self::select_bottom))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::submit))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .w_full()
            .px(px(11.))
            .py(px(6.))
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(if focused { t.accent } else { t.stroke_strong })
            .bg(if focused { t.layer } else { t.control })
            .when(!focused, |d| d.hover(move |s| s.bg(hover)))
            .font_family(FONT)
            .text_size(px(text::BODY.0))
            .line_height(px(text::BODY.1))
            .text_color(t.text)
            .child(FieldText { field: cx.entity(), colors })
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = &self.buf.text;
        let range = from_utf16(text, range_utf16.start)..from_utf16(text, range_utf16.end);
        actual_range.replace(to_utf16(text, range.start)..to_utf16(text, range.end));
        text.get(range).map(str::to_string)
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let text = &self.buf.text;
        let range = &self.buf.selected;
        Some(UTF16Selection { range: to_utf16(text, range.start)..to_utf16(text, range.end), reversed: self.buf.reversed })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let text = &self.buf.text;
        self.buf.marked.as_ref().map(|m| to_utf16(text, m.start)..to_utf16(text, m.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.buf.marked = None;
    }

    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new: &str, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.buf.range_from_utf16(range_utf16).or_else(|| self.buf.marked.clone());
        self.put(range, new, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.buf.range_from_utf16(range_utf16).or_else(|| self.buf.marked.clone()).unwrap_or_else(|| self.buf.selected.clone());
        self.buf.compose(range, new, new_selected_range_utf16);
        self.moved(cx);
        cx.emit(TextFieldEvent::Changed);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let l = self.layout.as_ref()?;
        let text = &self.buf.text;
        let start = l.position(from_utf16(text, range_utf16.start), self.downstream);
        let end = l.position(from_utf16(text, range_utf16.end), false);
        let width = if end.y == start.y { end.x - start.x } else { px(0.) };
        Some(Bounds::new(bounds.origin - l.scroll + start, size(width, l.line_height)))
    }

    fn character_index_for_point(&mut self, position: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let l = self.layout.as_ref()?;
        let (offset, _) = l.offset_at(position - l.bounds.origin + l.scroll);
        Some(to_utf16(&self.buf.text, self.buf.snap(offset)))
    }
}

/// The text and the selection, with no drawing, so the editing rules can be tested alone.
/// Offsets are in bytes and always on character boundaries.
#[derive(Default)]
struct Buffer {
    text: String,
    /// The caret is at its end, or at its start when `reversed`.
    selected: Range<usize>,
    reversed: bool,
    /// What the IME is still composing.
    marked: Option<Range<usize>>,
    multiline: bool,
    max_chars: usize,
}

impl Buffer {
    fn cursor(&self) -> usize {
        if self.reversed { self.selected.start } else { self.selected.end }
    }

    fn move_to(&mut self, offset: usize) {
        self.selected = offset..offset;
        self.reversed = false;
    }

    fn select_to(&mut self, offset: usize) {
        if self.reversed {
            self.selected.start = offset;
        } else {
            self.selected.end = offset;
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
    }

    fn select_all(&mut self) {
        self.selected = 0..self.text.len();
        self.reversed = false;
    }

    /// `offset` within the text and moved back onto a character boundary.
    fn snap(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn prev_boundary(&self, offset: usize) -> usize {
        self.text.grapheme_indices(true).rev().find_map(|(i, _)| (i < offset).then_some(i)).unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text.grapheme_indices(true).find_map(|(i, _)| (i > offset).then_some(i)).unwrap_or(self.text.len())
    }

    /// The start of the word before `offset`, skipping the spaces in between.
    fn prev_word(&self, offset: usize) -> usize {
        self.text[..offset].split_word_bound_indices().rev().find(|(_, w)| !w.trim().is_empty()).map_or(0, |(i, _)| i)
    }

    /// The end of the word after `offset`, skipping the spaces in between.
    fn next_word(&self, offset: usize) -> usize {
        self.text[offset..]
            .split_word_bound_indices()
            .find(|(_, w)| !w.trim().is_empty())
            .map_or(self.text.len(), |(i, w)| offset + i + w.len())
    }

    /// The word (or run of spaces, or punctuation mark) at `offset`, for a double click.
    fn word_at(&self, offset: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .map(|(i, w)| i..i + w.len())
            .find(|r| r.contains(&offset))
            .or_else(|| self.text.split_word_bound_indices().next_back().map(|(i, w)| i..i + w.len()))
            .unwrap_or(0..0)
    }

    /// Puts `new` in place of `range`, cleaned for this field and cut to the room the limit
    /// leaves, with the caret after it. Returns where the inserted text now is.
    fn replace(&mut self, range: Range<usize>, new: &str) -> Range<usize> {
        let range = self.snap(range.start.min(range.end))..self.snap(range.end.max(range.start));
        let new = clean(new, self.multiline);
        let kept = self.text.chars().count() - self.text[range.clone()].chars().count();
        let new = fit(&new, self.max_chars.saturating_sub(kept));
        self.text.replace_range(range.clone(), new);
        let inserted = range.start..range.start + new.len();
        self.move_to(inserted.end);
        self.marked = None;
        inserted
    }

    /// What the IME does while composing: puts `new` in place and marks it, with the selection
    /// it asks for given in UTF-16 and relative to `new`.
    fn compose(&mut self, range: Range<usize>, new: &str, selected_utf16: Option<Range<usize>>) {
        let inserted = self.replace(range, new);
        if let Some(sel) = selected_utf16 {
            let shown = &self.text[inserted.clone()];
            self.selected = inserted.start + from_utf16(shown, sel.start)..inserted.start + from_utf16(shown, sel.end);
            self.reversed = false;
        }
        self.marked = (!inserted.is_empty()).then_some(inserted);
    }

    fn range_from_utf16(&self, range: Option<Range<usize>>) -> Option<Range<usize>> {
        range.map(|r| from_utf16(&self.text, r.start)..from_utf16(&self.text, r.end))
    }
}

/// Text as a field holds it: line breaks as `\n`, or as spaces on one line, tabs as spaces, and
/// no other control characters.
fn clean(s: &str, multiline: bool) -> String {
    s.replace("\r\n", "\n")
        .chars()
        .filter_map(|c| match c {
            '\n' | '\r' if multiline => Some('\n'),
            '\n' | '\r' | '\t' => Some(' '),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

/// The longest start of `s` with at most `room` characters that does not split a grapheme.
fn fit(s: &str, room: usize) -> &str {
    let (mut chars, mut end) = (0, 0);
    for g in s.graphemes(true) {
        chars += g.chars().count();
        if chars > room {
            break;
        }
        end += g.len();
    }
    &s[..end]
}

/// IME offsets are in UTF-16 code units; the text is UTF-8.
fn from_utf16(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    for (i, c) in text.char_indices() {
        if utf16 >= offset {
            return i;
        }
        utf16 += c.len_utf16();
    }
    text.len()
}

fn to_utf16(text: &str, offset: usize) -> usize {
    text.char_indices().take_while(|&(i, _)| i < offset).map(|(_, c)| c.len_utf16()).sum()
}

#[derive(Clone, Copy)]
struct Colors {
    text: Hsla,
    placeholder: Hsla,
    selection: Hsla,
    caret: Hsla,
}

/// One row on screen, part or all of a line of the text.
struct Row {
    line: usize,
    /// Offsets in the whole text.
    start: usize,
    end: usize,
    /// Where the row starts in its unwrapped line.
    x0: Pixels,
}

/// The text as last drawn, to map offsets to points and back. Points are relative to the top
/// left of the text before scrolling.
struct Layout {
    lines: Vec<WrappedLine>,
    starts: Vec<usize>,
    rows: Vec<Row>,
    line_height: Pixels,
    /// The widest row.
    width: Pixels,
    bounds: Bounds<Pixels>,
    scroll: Point<Pixels>,
}

impl Layout {
    fn new(lines: Vec<WrappedLine>, line_height: Pixels, bounds: Bounds<Pixels>) -> Layout {
        let (mut starts, mut rows, mut width, mut start) = (Vec::new(), Vec::new(), px(0.), 0);
        for (i, line) in lines.iter().enumerate() {
            starts.push(start);
            let unwrapped = &line.unwrapped_layout;
            let (mut row_start, mut x0) = (0, px(0.));
            for b in line.wrap_boundaries() {
                let glyph = &unwrapped.runs[b.run_ix].glyphs[b.glyph_ix];
                rows.push(Row { line: i, start: start + row_start, end: start + glyph.index, x0 });
                width = width.max(glyph.position.x - x0);
                (row_start, x0) = (glyph.index, glyph.position.x);
            }
            rows.push(Row { line: i, start: start + row_start, end: start + line.len(), x0 });
            width = width.max(unwrapped.width - x0);
            start += line.len() + 1;
        }
        Layout { lines, starts, rows, line_height, width, bounds, scroll: Point::default() }
    }

    fn height(&self) -> Pixels {
        self.line_height * self.rows.len() as f32
    }

    fn x(&self, row: &Row, offset: usize) -> Pixels {
        self.lines[row.line].unwrapped_layout.x_for_index(offset - self.starts[row.line]) - row.x0
    }

    /// The row an offset is drawn on. Where a line wraps, the offset ends one row and starts
    /// the next; `downstream` picks the second.
    fn row_index(&self, offset: usize, downstream: bool) -> usize {
        let mut found = self.rows.len() - 1;
        for (i, r) in self.rows.iter().enumerate() {
            if (r.start..=r.end).contains(&offset) {
                found = i;
                let next_starts_here = self.rows.get(i + 1).is_some_and(|n| n.line == r.line && n.start == offset);
                if !(downstream && next_starts_here) {
                    break;
                }
            }
        }
        found
    }

    fn position(&self, offset: usize, downstream: bool) -> Point<Pixels> {
        let i = self.row_index(offset, downstream);
        let row = &self.rows[i];
        point(self.x(row, offset.clamp(row.start, row.end)), self.line_height * i as f32)
    }

    /// The offset closest to a point, and whether it is the start of a wrapped row.
    fn offset_at(&self, p: Point<Pixels>) -> (usize, bool) {
        let i = ((p.y / self.line_height).floor().max(0.) as usize).min(self.rows.len() - 1);
        let row = &self.rows[i];
        let line_start = self.starts[row.line];
        let offset = line_start + self.lines[row.line].unwrapped_layout.closest_index_for_x(p.x + row.x0);
        let offset = offset.clamp(row.start, row.end);
        (offset, offset == row.start && i > 0 && self.rows[i - 1].line == row.line)
    }

    /// The selection as one rectangle per row, reaching a little past the end of a row whose
    /// line break is selected.
    fn selection(&self, sel: &Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut out = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            let (s, e) = (sel.start.max(r.start), sel.end.min(r.end));
            let line_break = sel.end > r.end && r.line + 1 < self.lines.len() && self.rows.get(i + 1).is_none_or(|n| n.line != r.line);
            if s > e || (s == e && !line_break) {
                continue;
            }
            let x1 = self.x(r, e) + if line_break { px(6.) } else { px(0.) };
            let y = self.line_height * i as f32;
            out.push(Bounds::from_corners(point(self.x(r, s), y), point(x1, y + self.line_height)));
        }
        out
    }
}

/// What the field shows (its text, or the placeholder while empty), ready to shape.
#[derive(Clone)]
struct Shown {
    text: SharedString,
    runs: Vec<TextRun>,
    font_size: Pixels,
    multiline: bool,
}

fn shape(window: &Window, shown: &Shown, wrap: Option<Pixels>) -> Vec<WrappedLine> {
    window.text_system().shape_text(shown.text.clone(), shown.font_size, &shown.runs, wrap, None).map(|l| l.into_vec()).unwrap_or_default()
}

/// Room is left at the right for the caret after the last character.
fn wrap_width(width: Pixels) -> Pixels {
    (width - CARET_W).max(px(1.))
}

/// Draws the text, selection and caret, and takes the platform's text input while focused.
struct FieldText {
    field: Entity<TextField>,
    colors: Colors,
}

#[derive(Default)]
struct Prepaint {
    layout: Option<Layout>,
    selection: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

impl IntoElement for FieldText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for FieldText {
    type RequestLayoutState = Shown;
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Shown) {
        let field = self.field.read(cx);
        let style = window.text_style();
        let empty = field.buf.text.is_empty();
        let (text, color) =
            if empty { (field.placeholder.clone(), self.colors.placeholder) } else { (field.buf.text.clone().into(), self.colors.text) };
        let run = TextRun { len: text.len(), font: style.font(), color, background_color: None, underline: None, strikethrough: None };
        let runs = match field.buf.marked.clone().filter(|_| !empty) {
            Some(m) => {
                let underline = UnderlineStyle { color: Some(color), thickness: px(1.), wavy: false };
                [
                    TextRun { len: m.start, ..run.clone() },
                    TextRun { len: m.end - m.start, underline: Some(underline), ..run.clone() },
                    TextRun { len: text.len() - m.end, ..run },
                ]
                .into_iter()
                .filter(|r| r.len > 0)
                .collect()
            }
            None => vec![run],
        };
        let shown = Shown { text, runs, font_size: style.font_size.to_pixels(window.rem_size()), multiline: field.buf.multiline };
        let line_height = window.line_height();
        let mut layout = Style::default();
        layout.size.width = relative(1.).into();
        if !shown.multiline {
            layout.size.height = line_height.into();
            return (window.request_layout(layout, [], cx), shown);
        }
        let (min, max) = field.lines;
        let measured = shown.clone();
        let id = window.request_measured_layout(layout, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let rows =
                width.map_or(1, |w| shape(window, &measured, Some(wrap_width(w))).iter().map(|l| l.wrap_boundaries().len() + 1).sum());
            size(width.unwrap_or_default(), line_height * rows.clamp(min, max) as f32)
        });
        (id, shown)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        shown: &mut Shown,
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        let lines = shape(window, shown, shown.multiline.then(|| wrap_width(bounds.size.width)));
        if lines.is_empty() {
            return Prepaint::default();
        }
        let line_height = window.line_height();
        let mut layout = Layout::new(lines, line_height, bounds);
        let field = self.field.read(cx);
        let empty = field.buf.text.is_empty();
        let caret = layout.position(if empty { 0 } else { field.buf.cursor() }, field.downstream);

        let mut scroll = field.scroll;
        let view = bounds.size;
        if field.reveal {
            if shown.multiline {
                scroll.y = scroll.y.min(caret.y).max(caret.y + line_height - view.height);
            } else {
                scroll.x = scroll.x.min(caret.x).max(caret.x + CARET_W - view.width);
            }
        }
        scroll.x = scroll.x.min(layout.width + CARET_W - view.width).max(px(0.));
        scroll.y = scroll.y.min(layout.height() - view.height).max(px(0.));
        layout.scroll = scroll;

        let origin = bounds.origin - scroll;
        let selection = if empty { Vec::new() } else { layout.selection(&field.buf.selected) }
            .into_iter()
            .map(|r| fill(Bounds::new(origin + r.origin, r.size), self.colors.selection))
            .collect();
        let show_caret = field.focus.is_focused(window) && field.caret_on && field.buf.selected.is_empty();
        let caret = show_caret.then(|| fill(Bounds::new(origin + caret, size(CARET_W, line_height)), self.colors.caret));
        self.field.update(cx, |f, _| {
            f.scroll = scroll;
            f.reveal = false;
        });
        Prepaint { layout: Some(layout), selection, caret }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Shown,
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.field.read(cx).focus.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.field.clone()), cx);
        let Some(layout) = prepaint.layout.take() else { return };
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for quad in prepaint.selection.drain(..) {
                window.paint_quad(quad);
            }
            let lh = layout.line_height;
            let mut origin = bounds.origin - layout.scroll;
            for line in &layout.lines {
                let _ = line.paint(origin, lh, TextAlign::Left, None, window, cx);
                origin.y += line.size(lh).height;
            }
            if let Some(caret) = prepaint.caret.take() {
                window.paint_quad(caret);
            }
        });
        self.field.update(cx, |f, _| f.layout = Some(layout));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str, multiline: bool, max_chars: usize) -> Buffer {
        let mut b = Buffer { multiline, max_chars, ..Default::default() };
        b.replace(0..0, text);
        b
    }

    #[test]
    fn typing_and_deleting() {
        let mut b = buf("ação", false, 100);
        assert_eq!(b.cursor(), "ação".len());
        let at = b.cursor();
        b.replace(b.prev_boundary(at)..at, "");
        assert_eq!(b.text, "açã");
        b.move_to(1);
        b.replace(1..b.next_boundary(1), "");
        assert_eq!(b.text, "aã");
        b.replace(1..1, "é");
        assert_eq!((b.text.as_str(), b.cursor()), ("aéã", 1 + "é".len()));
    }

    #[test]
    fn graphemes_move_and_delete_whole() {
        let family = "👨\u{200d}👩\u{200d}👧";
        let b = buf(&format!("a{family}e\u{301}"), false, 100);
        assert_eq!(b.prev_boundary(b.text.len()), 1 + family.len());
        assert_eq!(b.prev_boundary(1 + family.len()), 1);
        assert_eq!(b.next_boundary(1), 1 + family.len());
        assert_eq!(b.next_boundary(b.text.len()), b.text.len());
        assert_eq!(b.prev_boundary(0), 0);
    }

    #[test]
    fn offsets_snap_onto_characters() {
        let mut b = buf("é", false, 100);
        assert_eq!(b.snap(1), 0);
        assert_eq!(b.snap(99), 2);
        b.replace(1..1, "x");
        assert_eq!(b.text, "xé");
    }

    #[test]
    fn limit_counts_characters() {
        let mut b = buf("abcd", false, 5);
        b.replace(4..4, "xyz");
        assert_eq!(b.text, "abcdx");
        b.replace(4..4, "z");
        assert_eq!(b.text, "abcdx");
        // Replacing a selection frees its room first.
        b.replace(0..2, "ççç");
        assert_eq!(b.text, "ççcdx");
        let b = buf("ãããããã", false, 5);
        assert_eq!(b.text.chars().count(), 5);
        // A grapheme that does not fit whole is left out whole.
        let b = buf("ae\u{301}", false, 2);
        assert_eq!(b.text, "a");
    }

    #[test]
    fn line_breaks_follow_the_kind_of_field() {
        assert_eq!(buf("a\r\nb\tc\rd\u{7}", false, 100).text, "a b c d");
        assert_eq!(buf("a\r\nb\tc\rd\u{7}", true, 100).text, "a\nb c\nd");
    }

    #[test]
    fn selection_flips_past_its_anchor() {
        let mut b = buf("hello", false, 100);
        b.move_to(3);
        b.select_to(5);
        assert_eq!((b.selected.clone(), b.cursor()), (3..5, 5));
        b.select_to(1);
        assert_eq!((b.selected.clone(), b.cursor(), b.reversed), (1..3, 1, true));
        b.select_all();
        assert_eq!(b.selected, 0..5);
    }

    #[test]
    fn words() {
        let b = buf("tudo bem, joão", false, 100);
        assert_eq!(b.prev_word(b.text.len()), "tudo bem, ".len());
        assert_eq!(b.prev_word("tudo bem, ".len()), "tudo bem".len());
        assert_eq!(b.next_word(0), "tudo".len());
        assert_eq!(b.next_word("tudo".len()), "tudo bem".len());
        assert_eq!(b.word_at(6), 5..8);
        assert_eq!(b.word_at(b.text.len()), "tudo bem, ".len()..b.text.len());
    }

    #[test]
    fn utf16_offsets() {
        let text = "a😀é";
        assert_eq!(to_utf16(text, 5), 3);
        assert_eq!(from_utf16(text, 3), 5);
        assert_eq!(to_utf16(text, text.len()), 4);
        assert_eq!(from_utf16(text, 4), text.len());
        assert_eq!(from_utf16(text, 99), text.len());
    }

    #[test]
    fn composing_marks_and_then_commits() {
        let mut b = buf("ab", false, 100);
        b.move_to(1);
        b.compose(1..1, "か", Some(1..1));
        assert_eq!((b.text.as_str(), b.marked.clone(), b.selected.clone()), ("aかb", Some(1..4), 4..4));
        let marked = b.marked.clone().unwrap();
        b.replace(marked, "火");
        assert_eq!((b.text.as_str(), b.marked.clone(), b.cursor()), ("a火b", None, 4));
    }
}
