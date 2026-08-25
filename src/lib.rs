pub mod animatable;
pub mod spring;
pub mod state;
pub mod tween;

#[cfg(feature = "gpui")]
mod element;
#[cfg(feature = "gpui")]
mod gpui_impls;
#[cfg(feature = "gpui")]
mod presence;
#[cfg(feature = "gpui")]
mod value;

pub use animatable::{Animatable, MAX_CHANNELS};
pub use spring::Spring;
pub use state::{MotionState, Transition};
pub use tween::{easing, Easing, Tween};

#[cfg(feature = "gpui")]
pub use element::{MotionElement, MotionExt};
#[cfg(feature = "gpui")]
pub use presence::presence;
#[cfg(feature = "gpui")]
pub use value::MotionValue;
