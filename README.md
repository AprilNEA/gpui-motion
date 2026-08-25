# gpui-motion

Framer Motion-style **state-driven** animation for [gpui](https://www.gpui.rs/) (Zed's Rust UI framework): springs, tweens, and velocity-preserving retargeting.

You declare a *target value* on every frame; the library smoothly drives the currently rendered value toward it. When the target changes mid-flight, the animation **keeps its current velocity and redirects seamlessly** — it never jumps and never restarts from the beginning.

```rust
use gpui_motion::{MotionExt, Spring};

div().with_motion(
    "panel",
    (
        px(if open { 340. } else { 0. }),
        if open { rgb(0x7c5cff).into() } else { rgb(0x2c2c38).into() },
    ),
    Spring::wobbly().into(),
    |el, (x, color): (Pixels, Rgba)| el.left(x).bg(color),
)
```

Click the toggle as fast as you like — the panel reverses mid-flight with its momentum intact.

## Why not `with_animation`?

gpui's built-in `with_animation` is **time-driven**: it plays a fixed `0 → 1` curve over a duration. If your state flips while the animation is running, you either restart from scratch (visual jump) or write bookkeeping by hand.

`gpui-motion` is **state-driven**: the source of truth is "where should this value be *now*", and the library owns the physics of getting there. Interruptions, reversals, and rapid target changes are the normal case, not an edge case.

## How it works

gpui rebuilds the element tree every frame. The only sanctioned way to keep data across frames is *element state*, keyed by a stable `ElementId`:

```diagram
┌──────────────────────┐   target per frame   ┌─────────────────────────┐
│ your render() code   │─────────────────────▶│ MotionElement           │
│ (declares targets)   │                      │  with_element_state     │
└──────────────────────┘                      │  ┌───────────────────┐  │
                                              │  │ MotionState       │  │
        Animatable: V ⇄ [f32; N]              │  │  x, v, target     │  │
        (N ≤ 8 channels)                      │  │  spring / tween   │  │
                                              │  └───────────────────┘  │
                                              │  !settled →             │
                                              │  request_animation_frame│
                                              └─────────────────────────┘
```

1. **`Animatable`** flattens a value into up to 8 `f32` channels (and rebuilds it afterwards). Implemented for `f32`, `Pixels`, `Rgba`, `Hsla`, `Point<Pixels>`, `Size<Pixels>`, and tuples of 2–6 `Animatable`s.
2. **The engine** advances each channel independently — a semi-implicit-Euler spring integrated at a fixed 240 Hz substep (frame gaps clamped to 1/30 s, so dropped frames can't blow up the integration), or a tween that re-anchors its `from` at every retarget and tracks velocity by finite differences so you can switch mid-flight from tween to spring without a discontinuity.
3. **The element layer** stores `MotionState` in gpui element state, ticks it during `request_layout`, transforms your child element with the current value, and calls `request_animation_frame()` until the animation settles. State lives and dies with the element — no global registry, no manual cleanup.

## API

### `with_motion` — animate any element

```rust
element.with_motion(id, target, transition, |el, value| ...)
    .initial(value)      // optional: mount-time start value (enter animation)
    .on_settle(|window, cx| ...)  // optional: fired once when the animation rests
```

### `presence` — enter/exit animation for one element

```rust
presence("toast", visible, enter_value, exit_value, transition, |value| render(value))
```

When `visible` turns `false` the element keeps rendering while it animates toward `exit_value`; only after settling does it become `Empty`. Flipping `visible` back during the exit reverses with preserved velocity.

### `MotionValue<T>` — a free-standing animated value

```rust
let progress = MotionValue::new(0.0f32, Spring::default().into());
progress.set_target(0.8);
let current = progress.get(window, cx); // advances; schedules a frame while unsettled
```

Store it in your `Entity`; clone it into closures (clones share state).

### Transitions

```rust
Spring::default()  // (170, 26) — react-spring "default"
Spring::gentle()   // (120, 14)
Spring::wobbly()   // (180, 12)
Spring::stiff()    // (310, 26)
Spring::slow()     // (280, 60)
Spring::new(stiffness, damping).rest(0.001, 0.001) // tighter rest thresholds (e.g. colors)

Tween::new(0.3)                          // seconds, smoothstep by default
Tween::new(0.3).easing(easing::ease_out_cubic)
```

The transition is passed on every frame and is *not* stored, so you can use different parameters per direction (e.g. `wobbly` to open, `stiff` to close).

## Framer Motion mapping

| Framer Motion | gpui-motion |
|---|---|
| `<motion.div animate={{ x, background }} />` | `div().with_motion(id, (x, color), t, ...)` |
| `transition={{ type: "spring", stiffness, damping }}` | `Spring::new(stiffness, damping).into()` |
| `transition={{ duration, ease }}` | `Tween::new(duration).easing(...).into()` |
| `initial={{ ... }}` | `.initial(value)` |
| `onAnimationComplete` | `.on_settle(...)` |
| `<AnimatePresence>` (single child) | `presence(id, visible, enter, exit, t, render)` |
| `useMotionValue` / `useSpring` | `MotionValue<T>` |
| `useReducedMotion` | automatic: `cx.reduce_motion()` ⇒ snap to target |

## Version notes

This crate pins gpui as a **git dependency on Zed's main branch** (the exact commit is resolved by your `Cargo.lock`):

```toml
gpui = { git = "https://github.com/zed-industries/zed" }
```

- Git dependencies only unify if downstream crates use the same source, so your app should depend on gpui the same way (a `[patch.crates-io]` entry works if some of your dependencies use the crates.io `gpui`).
- The element-layer API surface used here (`Element` trait with `inspector_id`, `Window::with_element_state`, `request_animation_frame`) is identical in crates.io `gpui 0.2.2`; the one thing 0.2.2 lacks is `App::reduce_motion` (added to gpui in mid-2026), so building against 0.2.2 requires removing that call.
- The demo additionally uses `gpui_platform` (windowing backends) — see `examples/demo`.

## Known limitations

- **No CSS-style transforms.** gpui's `div` has no `transform` property. Express translation via `left`/`top` (with absolute positioning) or margins, and scale via `w`/`h`. This is a framework fact, not a bug in this crate.
- **`ElementId` must be stable across frames.** Element state is keyed by id; an unstable id means the state is lost every frame and the animation restarts from scratch. In lists, key by your data (`("row", item.id)`), never by loop index.
- **At most 8 channels per animated value** (`MAX_CHANNELS`). Split larger values across multiple `with_motion` wrappers.
- **`Hsla` is interpolated by converting through `Rgba`** to avoid hue-wheel long-arc artifacts (red → blue passing through green). Alpha is interpolated as-is.
- **Reduced motion:** when the OS/user setting is on (`cx.reduce_motion()`), every animation snaps directly to its target. This is an accessibility requirement, not an option.

## Demo

```sh
cargo run -p demo
```

Three scenes: a spring panel (`with_motion` with a `(Pixels, Rgba)` tuple), a `presence` toast, and a `MotionValue` progress bar.

## Design decisions (for contributors)

- **Element state, not a global registry** — state is created and reclaimed with the element's lifecycle; the cost is the stable-`ElementId` requirement above.
- **The engine only knows `f32` channels** — `Animatable` does the (de)composition; colors go through linear `Rgba` space.
- **Transitions are caller-supplied per frame** — never persisted, so per-direction parameters are free.

## Non-goals for v0.1 (design keeps the door open)

FLIP layout animation, list-level presence, gesture bindings (`whileHover`/`whileTap`), and stagger. The velocity-preserving engine already supports the interruption semantics these need.
