use crate::{Icon, IconName, Sizable, Size};
use gpui::{
    Animation, AnimationExt as _, App, Hsla, IntoElement, ParentElement, RenderOnce, Styled as _,
    Transformation, Window, div, ease_in_out, percentage, prelude::FluentBuilder as _,
};
use instant::Duration;

/// How many poses a second the spinner turns at in a focused window. Uncapped,
/// it asks for every frame the display offers (240 a second on a 240 Hz
/// display) for a turn that 30 poses a second draws smoothly, 24 to a turn.
const POSES_PER_SECOND: f32 = 30.;

/// A cycling loading spinner.
#[derive(IntoElement)]
pub struct Spinner {
    size: Size,
    icon: Icon,
    speed: Duration,
    easing: Box<dyn Fn(f32) -> f32>,
    color: Option<Hsla>,
}

impl Spinner {
    /// Create a new loading spinner.
    pub fn new() -> Self {
        Self {
            size: Size::Medium,
            speed: Duration::from_secs_f64(0.8),
            easing: Box::new(ease_in_out),
            icon: Icon::new(IconName::Loader),
            color: None,
        }
    }

    /// Set specified icon for the spinner.
    ///
    /// Default is [`IconName::Loader`].
    ///
    /// Please ensure the icon used is suitable for a loading spinner.
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = icon.into();
        self
    }

    /// Set the icon color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Set the easing function.
    pub fn ease(mut self, easing: impl Fn(f32) -> f32 + 'static) -> Self {
        self.easing = Box::new(easing);
        self
    }
}

impl Sizable for Spinner {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let animation = Animation::new(self.speed).repeat().with_easing(self.easing);
        // A focused window takes a pose on the animation's own timer. An
        // unfocused one asks for frames the ordinary way: GPUI holds those to
        // its inactive frame interval (about 30 a second) and draws them with
        // the window's other motion, where the timer would add frames of its
        // own to a window GPUI is holding back.
        let animation = if window.is_window_active() {
            animation.with_max_fps(POSES_PER_SECOND)
        } else {
            animation
        };
        div()
            .child(
                self.icon
                    .with_size(self.size)
                    .when_some(self.color, |this, color| this.text_color(color))
                    .with_animation("circle", animation, |this, delta| {
                        this.transform(Transformation::rotate(percentage(delta)))
                    }),
            )
            .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Render, TestAppContext, px, size};
    use std::{cell::Cell, rc::Rc};

    struct SpinnerHost {
        renders: Rc<Cell<usize>>,
    }

    impl Render for SpinnerHost {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            Spinner::new()
        }
    }

    fn open_host(cx: &mut TestAppContext) -> (Rc<Cell<usize>>, gpui::WindowHandle<SpinnerHost>) {
        let renders = Rc::new(Cell::new(0));
        let host = renders.clone();
        let window = cx.open_window(size(px(100.), px(100.)), |_, _| SpinnerHost {
            renders: host,
        });
        // Test windows open unfocused. Focus this one and draw the spinner again.
        window
            .update(cx, |_, window, _| {
                window.activate_window();
                window.refresh();
            })
            .unwrap();
        cx.run_until_parked();
        (renders, window)
    }

    #[gpui::test]
    fn reduced_motion_spinner_is_static_and_requests_no_frame(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_reduce_motion(true));
        let (_, window) = open_host(cx);

        assert_eq!(
            window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap(),
            0
        );
    }

    /// A 240 Hz display offers a frame every 4.17 ms. The spinner takes a
    /// pose in 30 of the 240 each second, on its own timer, and asks for no
    /// frame of its own.
    #[gpui::test]
    fn spinner_turns_at_thirty_poses_a_second_on_a_240_hz_display(cx: &mut TestAppContext) {
        let (renders, window) = open_host(cx);
        let before = renders.get();

        for _ in 0..240 {
            cx.executor().advance_clock(Duration::from_nanos(4_166_667));
            window
                .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }

        let poses = renders.get() - before;
        assert!(
            (29..=31).contains(&poses),
            "a second of a 240 Hz display drew {poses} poses"
        );
    }

    /// An unfocused window asks for the spinner's frames the ordinary way,
    /// which GPUI holds to its inactive frame interval, and the focused
    /// window's spinner asks for none: its poses come on a timer.
    #[gpui::test]
    fn spinner_asks_for_frames_only_while_its_window_is_unfocused(cx: &mut TestAppContext) {
        let renders = Rc::new(Cell::new(0));
        let host = renders.clone();
        let (_, cx) = cx.add_window_view(|_, _| SpinnerHost { renders: host });
        cx.update(|window, _| {
            window.activate_window();
            window.refresh();
        });
        cx.run_until_parked();
        let asked = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, cx| window.simulate_next_frame(cx))
        };

        // The window's first draw, unfocused, asked for frames: let them pass,
        // then draw the focused spinner afresh.
        asked(cx);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();

        assert!(cx.update(|window, _| window.is_window_active()));
        assert_eq!(asked(cx), 0);

        cx.deactivate_window();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();

        assert!(!cx.update(|window, _| window.is_window_active()));
        assert!(asked(cx) > 0);
    }
}
