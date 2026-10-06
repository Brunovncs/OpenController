//! "Open Controller 0.3.0 is out": a bar under the top bar when GitHub has a newer release. On
//! Windows "Update now" downloads the installer, checks it, asks the resident process to quit
//! (which unplugs the virtual controllers and shows the hidden ones again) and runs it; the
//! installer replaces the programs in place and starts Open Controller again. Elsewhere the bar
//! opens the release's page.

use crate::theme::{icon as glyph, radius};
use crate::ui::MainView;
use crate::widgets::{Kind, body, button, caption, icon, icon_button};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, px};
use open_controller_core::i18n::fill;
use open_controller_core::update::{self, Release};
use std::sync::atomic::{AtomicBool, Ordering};

/// Set while "Update now" has asked the resident process to quit and is about to start the
/// installer, so the window does not close with it.
static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn installing() -> bool {
    INSTALLING.load(Ordering::SeqCst)
}

#[derive(Clone, Default, PartialEq)]
// Only Windows installs updates itself, so only it works on one or fails.
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Update {
    #[default]
    Unknown,
    /// Asked, and this is the latest.
    Current,
    Available(Release),
    Working(Release),
    Failed(Release, String),
    Dismissed,
}

impl MainView {
    /// Asks GitHub once, when the preferences say so (they arrive from the resident process a
    /// moment after the window opens).
    pub fn maybe_check_updates(&mut self, cx: &mut Context<Self>) {
        let m = self.model.read(cx);
        if self.update != Update::Unknown || !m.prefs.check_updates || m.demo {
            return;
        }
        self.update = Update::Current;
        cx.spawn(async move |this, cx| {
            let found = cx.background_executor().spawn(async { update::check() }).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(Some(r)) = found {
                    this.update = Update::Available(r);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    #[cfg(windows)]
    fn update_now(&mut self, r: Release, cx: &mut Context<Self>) {
        self.update = Update::Working(r.clone());
        cx.notify();
        let dir = std::env::temp_dir().join("open-controller-update");
        cx.spawn(async move |this, cx| {
            let got = cx.background_executor().spawn({
                let r = r.clone();
                async move { update::download_setup(&r, &dir) }
            });
            let file = match got.await {
                Ok(f) => f,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| {
                        this.update = Update::Failed(r, e);
                        cx.notify();
                    });
                    return;
                }
            };
            INSTALLING.store(true, Ordering::SeqCst);
            let _ = this.update(cx, |this, cx| this.send(open_controller_core::ipc::ToTray::Quit, cx));
            let started = cx
                .background_executor()
                .spawn(async move {
                    open_controller_core::instance::wait_gone(crate::RESIDENT, std::time::Duration::from_secs(15));
                    update::run_setup(&file)
                })
                .await;
            let _ = this.update(cx, |this, cx| match started {
                Ok(()) => cx.quit(),
                Err(e) => {
                    INSTALLING.store(false, Ordering::SeqCst);
                    this.update = Update::Failed(r, e);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Installs it on Windows when the release has an installer; elsewhere opens its page.
    fn update_action(&self, r: &Release, cx: &mut Context<Self>) -> AnyElement {
        let (t, text) = (self.theme, self.model.read(cx).text);
        #[cfg(windows)]
        if r.setup.is_some() {
            let r = r.clone();
            return button("update-now", text.update_now, Kind::Primary, &t)
                .on_click(cx.listener(move |this, _, _, cx| this.update_now(r.clone(), cx)))
                .into_any_element();
        }
        let page = r.page.clone();
        button("update-download", text.update_download, Kind::Primary, &t).on_click(move |_, _, cx| cx.open_url(&page)).into_any_element()
    }

    pub fn update_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (r, working, failed) = match &self.update {
            Update::Available(r) => (r.clone(), false, None),
            Update::Working(r) => (r.clone(), true, None),
            Update::Failed(r, e) => (r.clone(), false, Some(e.clone())),
            _ => return None,
        };
        let t = self.theme;
        let text = self.model.read(cx).text;
        let page = r.page.clone();
        let notes = div()
            .id("update-notes")
            .flex()
            .items_center()
            .gap(px(4.))
            .cursor_pointer()
            .on_click(move |_, _, cx| cx.open_url(&page))
            .child(caption(text.update_notes, t.accent))
            .child(icon(glyph::OPEN, 10., t.accent));
        let action = if working {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon(glyph::CLOCK, 14., t.text2))
                .child(body(text.update_working, t.text2))
                .into_any_element()
        } else {
            self.update_action(&r, cx)
        };
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(14.))
                .px(px(18.))
                .py(px(10.))
                .bg(t.accent_soft)
                .border_b_1()
                .border_color(t.stroke)
                .child(icon(glyph::INFO, 16., t.accent))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(2.))
                        .child(body(fill(text.app_update, &r.version), t.text))
                        .when_some(failed, |d, e| d.child(caption(fill(text.update_failed, e), t.critical))),
                )
                .child(notes)
                .child(action)
                .when(!working, |d| {
                    d.child(icon_button("update-dismiss", glyph::CLOSE, &t).rounded(px(radius::CONTROL)).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.update = Update::Dismissed;
                            cx.notify();
                        },
                    )))
                })
                .into_any_element(),
        )
    }
}
