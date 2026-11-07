//! Help for the combination-mode editor.
use crate::{
    ui::MainView,
    widgets::{Kind, button, caption, strong},
};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px};
impl MainView {
    pub fn open_combination_help(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.stop_press_meter(cx);
        self.stop_shortcut_recording(cx);
        self.stop_shortcut_key_capture(cx);
        if let Some(e) = self.editor.as_mut() {
            e.recording = false;
        }
        self.combination_help = true;
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub fn render_combination_help(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.combination_help {
            return None;
        }
        let t = self.theme;
        let text = &self.model.read(cx).text.combinations;
        let content = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(div().flex().items_center().gap(px(12.)).child(strong(text.help_title, t.text).flex_1()).child(
                button("help-close-top", "×", Kind::Subtle, &t).on_click(cx.listener(|this, _, _, cx| {
                    this.combination_help = false;
                    cx.notify();
                })),
            ))
            .children(
                text.help_sections
                    .iter()
                    .map(|(title, body)| div().flex().flex_col().gap(px(6.)).child(strong(*title, t.text)).child(caption(*body, t.text2))),
            )
            .child(button("help-close", text.help_close, Kind::Primary, &t).on_click(cx.listener(|this, _, _, cx| {
                this.combination_help = false;
                cx.notify();
            })));
        Some(
            div()
                .id("combination-help-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .bg(gpui::black().opacity(0.55))
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.combination_help = false;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("combination-help-dialog")
                        .w_full()
                        .max_w(px(660.))
                        .max_h_full()
                        .overflow_y_scroll()
                        .p(px(24.))
                        .rounded(px(16.))
                        .bg(t.layer)
                        .border_1()
                        .border_color(t.stroke_strong)
                        .shadow_lg()
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(content),
                )
                .into_any_element(),
        )
    }
}
