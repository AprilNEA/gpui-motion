pub mod animatable;
pub mod inertia;
pub mod spring;
pub mod state;
pub mod transition;
pub mod tween;

#[cfg(feature = "gpui")]
mod drag;
#[cfg(feature = "gpui")]
mod element;
#[cfg(feature = "gpui")]
mod flip;
#[cfg(feature = "gpui")]
mod gpui_impls;
#[cfg(feature = "gpui")]
mod presence;
#[cfg(feature = "gpui")]
mod scope;
#[cfg(feature = "gpui")]
mod value;

pub use animatable::{Animatable, MAX_CHANNELS};
pub use inertia::Inertia;
pub use spring::Spring;
pub use state::MotionState;
pub use transition::{
    ChannelTransitions, EasingSeq, KeyframesTiming, MAX_KEYFRAMES, Repeat, RepeatCount, RepeatKind,
    Transition, TransitionKind,
};
pub use tween::{Easing, Tween, easing};

#[cfg(feature = "gpui")]
pub use drag::DragTracker;
#[cfg(feature = "gpui")]
pub use element::{IntoMotionTarget, IntoTransitions, MotionElement, MotionExt};
#[cfg(feature = "gpui")]
pub use flip::{Flip, FlipExt};
#[cfg(feature = "gpui")]
pub use presence::{PresenceGroup, PresenceMode, presence, presence_group};
#[cfg(feature = "gpui")]
pub use scope::{Variants, When};
#[cfg(feature = "gpui")]
pub use value::MotionValue;

#[cfg(all(feature = "gpui", feature = "gpui-main"))]
pub(crate) fn reduce_motion(cx: &gpui::App) -> bool {
    cx.reduce_motion()
}

#[cfg(all(feature = "gpui", not(feature = "gpui-main")))]
pub(crate) fn reduce_motion(_cx: &gpui::App) -> bool {
    // gpui 0.2.2 cannot expose the system preference, so retain normal animation behavior.
    false
}
