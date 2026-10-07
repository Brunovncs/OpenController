//! The window's own frame, where the desktop leaves it to the program, as GNOME does on Wayland:
//! a shadow, edges to resize the window by, and minimize, maximize and close in the bar on top.
//! Where the system draws the frame (Windows, macOS, X11, KDE) none of this shows.

use crate::theme::{Theme, icon as glyph, layout, radius};
use crate::ui::MainView;
use crate::widgets::{icon, icon_button};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, BoxShadow, Context, CursorStyle, Decorations, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, ResizeEdge, Stateful, StatefulInteractiveElement, Styled, Tiling, Window, WindowButton,
    WindowButtonLayout, div, hsla, point, px, rgb,
};

/// How far the shadow and the resize edges reach around the window.
const INSET: f32 = 10.;
const BUTTON: f32 = 36.;
const GAP: f32 = 4.;
/// From the window's buttons to the side of the window.
const EDGE: f32 = 10.;

/// Which sides of the window touch the screen's edges or another window, when the program
/// draws the frame; `None` when the system does.
pub fn tiling(window: &Window) -> Option<Tiling> {
    match window.window_decorations() {
        Decorations::Client { tiling } => Some(tiling),
        Decorations::Server => None,
    }
}

/// The page inside the frame: a hairline edge and a shadow on the free sides, and handles there
/// to resize the window by.
pub fn frame(page: AnyElement, t: &Theme, window: &mut Window) -> AnyElement {
    let Some(tiling) = tiling(window) else { return page };
    window.set_client_inset(px(INSET));
    let shadow = BoxShadow {
        color: hsla(0., 0., 0., 0.3),
        offset: point(px(0.), px(1.)),
        blur_radius: px(INSET / 2.),
        spread_radius: px(0.),
        inset: false,
    };
    let body = div()
        .size_full()
        .border_color(t.stroke_strong)
        .when(!tiling.top, |d| d.border_t_1())
        .when(!tiling.bottom, |d| d.border_b_1())
        .when(!tiling.left, |d| d.border_l_1())
        .when(!tiling.right, |d| d.border_r_1())
        .when(!tiling.is_tiled(), |d| d.shadow(vec![shadow]))
        .child(page);
    let edges = [
        (ResizeEdge::Top, !tiling.top),
        (ResizeEdge::Bottom, !tiling.bottom),
        (ResizeEdge::Left, !tiling.left),
        (ResizeEdge::Right, !tiling.right),
        (ResizeEdge::TopLeft, !tiling.top && !tiling.left),
        (ResizeEdge::TopRight, !tiling.top && !tiling.right),
        (ResizeEdge::BottomLeft, !tiling.bottom && !tiling.left),
        (ResizeEdge::BottomRight, !tiling.bottom && !tiling.right),
    ];
    div()
        .relative()
        .size_full()
        .when(!tiling.top, |d| d.pt(px(INSET)))
        .when(!tiling.bottom, |d| d.pb(px(INSET)))
        .when(!tiling.left, |d| d.pl(px(INSET)))
        .when(!tiling.right, |d| d.pr(px(INSET)))
        .child(body)
        .children(edges.into_iter().filter(|(_, free)| *free).map(|(edge, _)| handle(edge)))
        .into_any_element()
}

/// An invisible strip along one edge, or a square on a corner, that resizes the window.
fn handle(edge: ResizeEdge) -> Div {
    let corner = INSET * 1.5;
    let d = div().absolute().on_mouse_down(MouseButton::Left, move |_, window, cx| {
        cx.stop_propagation();
        window.start_window_resize(edge);
    });
    match edge {
        ResizeEdge::Top => d.top_0().left_0().right_0().h(px(INSET)).cursor(CursorStyle::ResizeUpDown),
        ResizeEdge::Bottom => d.bottom_0().left_0().right_0().h(px(INSET)).cursor(CursorStyle::ResizeUpDown),
        ResizeEdge::Left => d.top_0().bottom_0().left_0().w(px(INSET)).cursor(CursorStyle::ResizeLeftRight),
        ResizeEdge::Right => d.top_0().bottom_0().right_0().w(px(INSET)).cursor(CursorStyle::ResizeLeftRight),
        ResizeEdge::TopLeft => d.top_0().left_0().size(px(corner)).cursor(CursorStyle::ResizeUpLeftDownRight),
        ResizeEdge::BottomRight => d.bottom_0().right_0().size(px(corner)).cursor(CursorStyle::ResizeUpLeftDownRight),
        ResizeEdge::TopRight => d.top_0().right_0().size(px(corner)).cursor(CursorStyle::ResizeUpRightDownLeft),
        ResizeEdge::BottomLeft => d.bottom_0().left_0().size(px(corner)).cursor(CursorStyle::ResizeUpRightDownLeft),
    }
}

/// The desktop's window buttons for each side of the bar, without the ones it cannot do.
fn buttons(window: &Window, cx: &App) -> (Vec<WindowButton>, Vec<WindowButton>) {
    let layout = cx.button_layout().unwrap_or(WindowButtonLayout {
        left: [None; 3],
        right: [Some(WindowButton::Minimize), Some(WindowButton::Maximize), Some(WindowButton::Close)],
    });
    let can = window.window_controls();
    let keep = |side: [Option<WindowButton>; 3]| {
        side.into_iter()
            .flatten()
            .filter(|b| match b {
                WindowButton::Minimize => can.minimize,
                WindowButton::Maximize => can.maximize,
                WindowButton::Close => true,
            })
            .collect::<Vec<_>>()
    };
    (keep(layout.left), keep(layout.right))
}

impl MainView {
    /// The bar on top, made the window's title bar when the program draws the frame: dragging it
    /// moves the window, a double click maximizes, a right click opens the window menu, and the
    /// window's buttons sit at its ends. `row` is the bar's content, kept clear of them.
    pub fn title_bar(&self, bar: Div, row: Div, window: &Window, cx: &mut Context<Self>) -> Div {
        let Some(tiling) = tiling(window) else { return bar.child(row) };
        let t = self.theme;
        let (left, right) = buttons(window, cx);
        let can = window.window_controls();
        let maximized = window.is_maximized();
        let free = |tiled: bool| if tiled { 0. } else { INSET + 1. };
        let width = f32::from(window.viewport_size().width) - free(tiling.left) - free(tiling.right);
        let margin = (width - width.min(layout::MAX_W)) / 2.;
        let room = |n: usize| {
            let own = layout::GUTTER - 6.;
            if n == 0 { own } else { own.max(EDGE + n as f32 * (BUTTON + GAP) + 4. - margin) }
        };
        let row = row.pl(px(room(left.len()))).pr(px(room(right.len())));
        let mut side = |list: Vec<WindowButton>| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .gap(px(GAP))
                .children(list.into_iter().map(|b| window_button(b, maximized, &t, cx)))
        };
        let (left, right) = (side(left), side(right));
        bar.relative()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, _| {
                    this.moving = ev.click_count < 2;
                    if ev.click_count == 2 && can.maximize {
                        window.zoom_window();
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, window, _| {
                if std::mem::take(&mut this.moving) && ev.pressed_button == Some(MouseButton::Left) {
                    window.start_window_move();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.moving = false))
            .on_mouse_down_out(cx.listener(|this, _, _, _| this.moving = false))
            .when(can.window_menu, |d| {
                d.on_mouse_down(MouseButton::Right, |ev: &MouseDownEvent, window, _| window.show_window_menu(ev.position))
            })
            .child(row)
            .child(left.left(px(EDGE)))
            .child(right.right(px(EDGE)))
    }
}

/// Minimize, maximize or close, in the style of the window's other icon buttons; close turns red.
fn window_button(b: WindowButton, maximized: bool, t: &Theme, cx: &mut Context<MainView>) -> Stateful<Div> {
    let d = match b {
        WindowButton::Minimize => icon_button(b.id(), glyph::MINIMIZE, t),
        WindowButton::Maximize => icon_button(b.id(), if maximized { glyph::RESTORE } else { glyph::MAXIMIZE }, t),
        WindowButton::Close => {
            let red = rgb(0xc42b1c);
            let at = (BUTTON - 16.) / 2.;
            div()
                .id(b.id())
                .group(b.id())
                .relative()
                .flex_none()
                .size(px(BUTTON))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .hover(move |s| s.bg(red))
                .child(icon(glyph::CLOSE, 16., t.text).absolute().top(px(at)).left(px(at)).group_hover(b.id(), |s| s.invisible()))
                .child(
                    icon(glyph::CLOSE, 16., hsla(0., 0., 1., 1.))
                        .absolute()
                        .top(px(at))
                        .left(px(at))
                        .invisible()
                        .group_hover(b.id(), |s| s.visible()),
                )
        }
    };
    d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).on_click(cx.listener(move |_, _, window, _| match b {
        WindowButton::Minimize => window.minimize_window(),
        WindowButton::Maximize => window.zoom_window(),
        WindowButton::Close => window.remove_window(),
    }))
}
