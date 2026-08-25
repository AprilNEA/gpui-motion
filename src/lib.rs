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
    ChannelTransitions, EasingSeq, KeyframesTiming, Repeat, RepeatCount, RepeatKind, Transition,
    TransitionKind, MAX_KEYFRAMES,
};
pub use tween::{easing, Easing, Tween};

#[cfg(feature = "gpui")]
pub use drag::DragTracker;
#[cfg(feature = "gpui")]
pub use element::{IntoMotionTarget, IntoTransitions, MotionElement, MotionExt};
#[cfg(feature = "gpui")]
pub use flip::{Flip, FlipExt};
#[cfg(feature = "gpui")]
pub use presence::{presence, presence_group, PresenceGroup, PresenceMode};
#[cfg(feature = "gpui")]
pub use scope::{Variants, When};
#[cfg(feature = "gpui")]
pub use value::MotionValue;
