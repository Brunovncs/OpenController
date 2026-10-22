//! Mapping a joystick SDL has no layout for, on its page: each part asked for in turn and shown
//! held in the drawing, and the mapping made from what it sent kept by the resident process.

use crate::theme::{icon as glyph, radius};
use crate::ui::MainView;
use crate::widgets::{Kind, body, button, caption, icon, strong};
use gpui::prelude::FluentBuilder;
use gpui::{Context, Div, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::i18n::{Text, fill};
use open_controller_core::ipc::ToTray;
use open_controller_core::learn::{Learning, STEPS, Show, Want};
use open_controller_core::mapping::PadState;
use open_controller_core::{PadKey, PadView, Role};
use std::time::{Duration, Instant};

/// How long a page waits for its controller to come back under another key.
const FOLLOW_WAIT: Duration = Duration::from_secs(5);

/// A joystick being mapped on its page.
pub struct Learn {
    pub key: PadKey,
    guid: String,
    name: String,
    learning: Learning,
}

/// A controller whose page should open once it shows up under another key: a gamepad once
/// mapped, or unmapped again to be mapped anew.
pub struct Follow {
    guid: String,
    unmapped: bool,
    since: Instant,
}

impl MainView {
    pub fn start_learning(&mut self, pad: &PadView, cx: &mut Context<Self>) {
        // What it sends at rest, as it was left alone before the first step.
        let Some(rest) = pad.raw.clone() else { return };
        self.learn = Some(Learn { key: pad.key, guid: pad.guid.clone(), name: pad.name.clone(), learning: Learning::new(rest) });
        cx.notify();
    }

    /// Takes in what the joystick on the open page sends.
    pub fn feed_learning(&mut self, pad: &PadView) {
        if let (Some(l), Some(raw)) = (self.learn.as_mut().filter(|l| l.key == pad.key), &pad.raw) {
            l.learning.feed(raw);
        }
    }

    fn save_learning(&mut self, cx: &mut Context<Self>) {
        let Some(l) = self.learn.take().filter(|l| l.learning.any()) else { return };
        self.follow = Some(Follow { guid: l.guid.clone(), unmapped: false, since: Instant::now() });
        self.send(ToTray::Map { guid: l.guid, name: l.name, fields: l.learning.fields() }, cx);
        cx.notify();
    }

    /// Takes away the mapping its owner made, and maps it again once it is unmapped.
    pub fn map_again(&mut self, pad: &PadView, cx: &mut Context<Self>) {
        self.follow = Some(Follow { guid: pad.guid.clone(), unmapped: true, since: Instant::now() });
        self.send(ToTray::Unmap(pad.guid.clone()), cx);
    }

    /// Opens the page of the controller being followed once it shows up. Whether the screen is
    /// left to wait for it.
    pub fn follow(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(f) = &self.follow else { return false };
        let snap = self.model.read(cx).snapshot.clone();
        let Some(pad) = snap.pads.iter().find(|p| p.guid == f.guid && (p.role == Role::Unmapped) == f.unmapped) else {
            if f.since.elapsed() > FOLLOW_WAIT {
                self.follow = None;
                return false;
            }
            return true;
        };
        let again = f.unmapped;
        self.follow = None;
        self.open(pad.key, cx);
        if again {
            self.start_learning(pad, cx);
        }
        true
    }

    /// The drawing while mapping: the part asked for, held.
    pub fn learning_pose(&self, pad: &PadView) -> Option<PadState> {
        let l = self.learn.as_ref().filter(|l| l.key == pad.key && !l.learning.held())?;
        let step = STEPS.get(l.learning.step())?;
        let mut pose = PadState::default();
        match step.show {
            Show::Button(b) => pose.buttons = 1 << b,
            Show::Axis(a) => pose.axes[a] = i16::MAX,
        }
        Some(pose)
    }

    /// In place of the notice on an unmapped gamepad's page: what to do now, and the buttons to
    /// skip a part, go back, start over, save or stop.
    pub fn learn_block(&self, pad: &PadView, text: &'static Text, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let key = pad.key;
        let shell = div().flex().flex_col().gap(px(10.)).mb(px(16.)).px(px(14.)).py(px(12.)).rounded(px(radius::CARD)).bg(t.accent_soft);
        let Some(l) = self.learn.as_ref().filter(|l| l.key == key) else {
            return shell.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.))
                    .child(icon(glyph::INFO, 14., t.accent))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(240.))
                            .child(strong(text.unknown_title, t.text))
                            .child(caption(text.learn_offer, t.text2)),
                    )
                    .child(button("learn", text.learn_start, Kind::Primary, &t).on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(pad) = this.pad(key, cx) {
                            this.start_learning(&pad, cx);
                        }
                    })))
                    .child(
                        button("unknown-send", text.unknown_send, Kind::Standard, &t)
                            .on_click(cx.listener(move |this, _, w, cx| this.open_report(key, true, w, cx))),
                    ),
            );
        };
        let learning = &l.learning;
        let step = learning.step();
        let mut lines = div().flex().flex_col().gap(px(4.));
        let mut actions = div().flex().flex_wrap().items_center().gap(px(8.));
        if learning.done() {
            let any = learning.any();
            lines = lines.child(body(if any { text.learn_done } else { text.learn_nothing }, t.text));
            actions = actions
                .when(any, |d| {
                    d.child(
                        button("learn-save", text.learn_save, Kind::Primary, &t)
                            .on_click(cx.listener(|this, _, _, cx| this.save_learning(cx))),
                    )
                })
                .child(button("learn-back", text.learn_back, Kind::Standard, &t).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(l) = this.learn.as_mut() {
                        l.learning.back();
                    }
                    cx.notify();
                })))
                .child(button("learn-again", text.learn_again, Kind::Standard, &t).on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(pad) = this.pad(key, cx) {
                        this.start_learning(&pad, cx);
                    }
                })));
        } else {
            let s = &STEPS[step];
            let ask = if s.want == Want::Push { text.learn_push } else { text.learn_press };
            lines = lines
                .child(caption(fill(&fill(text.learn_count, step + 1), STEPS.len()), t.text2))
                .child(body(fill(ask, text.learn_parts[step]), t.text).font_weight(gpui::FontWeight::SEMIBOLD));
            if learning.held()
                && let Some(got) = step.checked_sub(1).and_then(|s| learning.bound(s))
            {
                lines = lines.child(caption(fill(text.learn_got, got), t.success));
            }
            actions = actions
                .child(button("learn-skip", text.learn_skip, Kind::Standard, &t).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(l) = this.learn.as_mut() {
                        l.learning.skip();
                    }
                    cx.notify();
                })))
                .when(step > 0, |d| {
                    d.child(button("learn-back", text.learn_back, Kind::Standard, &t).on_click(cx.listener(|this, _, _, cx| {
                        if let Some(l) = this.learn.as_mut() {
                            l.learning.back();
                        }
                        cx.notify();
                    })))
                });
        }
        let actions = actions.child(div().flex_1()).child(button("learn-cancel", text.learn_cancel, Kind::Subtle, &t).on_click(
            cx.listener(|this, _, _, cx| {
                this.learn = None;
                cx.notify();
            }),
        ));
        shell
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(icon(glyph::BUTTONS, 14., t.accent))
                    .child(div().flex().flex_col().child(strong(text.learn_title, t.text)).child(caption(text.learn_desc, t.text2))),
            )
            .child(lines)
            .child(actions)
    }
}
