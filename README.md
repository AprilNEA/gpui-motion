# gpui-motion

Framer Motion-style **state-driven** animation for [gpui](https://www.gpui.rs/) (Zed's Rust UI framework): closed-form springs, tweens, keyframes, inertia, gestures, and velocity-preserving retargeting.

You declare a *target value* on every frame; the library smoothly drives the currently rendered value toward it. When the target changes mid-flight, the animation **keeps its current velocity and redirects seamlessly** — it never jumps and never restarts from the beginning.

```rust
use gpui_motion::{MotionExt, Spring};

div().with_motion(
    "panel",
    (
        px(if open { 340. } else { 0. }),
        if open { rgb(0x7c5cff).into() } else { rgb(0x2c2c38).into() },
    ),
    Spring::wobbly(),
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
        (N ≤ 8 channels)                      │  │  per-channel track│  │
                                              │  └───────────────────┘  │
                                              │  !settled →             │
                                              │  request_animation_frame│
                                              └─────────────────────────┘
```

1. **`Animatable`** flattens a value into up to 8 `f32` channels (and rebuilds it afterwards). Implemented for `f32`, `Pixels`, `Rgba`, `Hsla`, `Point<Pixels>`, `Size<Pixels>`, and tuples of 2–6 `Animatable`s.
2. **The engine** advances each channel independently. Springs use the closed-form analytic solution across all three damping branches (under-, critically-, and over-damped), so they are exact at any frame rate — no fixed-timestep integration, no divergence after dropped frames. Tweens re-anchor their `from` at every retarget and track velocity by finite differences, so switching mid-flight from tween to spring keeps momentum.
3. **The element layer** stores `MotionState` in gpui element state, ticks it during `request_layout`, transforms your child element with the current value, and calls `request_animation_frame()` until the animation settles. State lives and dies with the element — no global registry, no manual cleanup.

## API

### `with_motion` — animate any element

```rust
element.with_motion(id, target, transition, |el, value| ...)
    .initial(value)                // optional: mount-time start value (enter animation)
    .while_hover(value)            // optional: target while hovered
    .while_press(value)            // optional: target while pressed (wins over hover)
    .on_settle(|window, cx| ...)   // optional: fired once when the animation rests
```

- `target` is a single value **or keyframes** (`[V; N]` / `Vec<V>`, up to 8 frames).
- `transition` is anything convertible to a `Transition` (`Spring`, `Tween`, `KeyframesTiming`, `Inertia`) — or a **tuple of transitions**, one per tuple element of the animated value (per-property transitions):

```rust
// x springs, color tweens — one transition per tuple element
div().with_motion(
    "panel",
    (target_x, target_color),
    (Spring::stiff(), Tween::new(0.25)),
    |el, (x, color): (Pixels, Rgba)| el.left(x).bg(color),
)
```

### Gestures — `while_hover` / `while_press`

```rust
div().with_motion("button", base_size, Spring::stiff(), |el, s: Pixels| el.w(s).h(s))
    .while_hover(px(56.))
    .while_press(px(44.))
```

Priority is press > hover > base. The element inserts its own hitbox and mouse listeners; entering/leaving mid-animation redirects with preserved velocity, like everything else.

### Keyframes

```rust
div().with_motion(
    "pulse",
    [px(40.), px(64.), px(40.)],                        // keyframe values
    Transition::from(
        KeyframesTiming::new(1.2)                       // total duration (seconds)
            .times(&[0.0, 0.3, 1.0])                    // optional offsets (0..=1)
            .easings(&[easing::ease_out, easing::ease_in]), // per-segment easing
    )
    .repeat_forever(RepeatKind::Mirror),
    |el, s: Pixels| el.w(s).h(s),
)
```

Retargeting keyframes mid-flight re-anchors the first frame at the current value — no jump.

### Transitions: delay and repeat

Every `Transition` supports:

```rust
Transition::from(Tween::new(0.4))
    .delay(0.2)                                 // seconds before starting
    .repeat_times(3, RepeatKind::Reverse)       // Loop | Reverse | Mirror
// or .repeat_forever(RepeatKind::Loop)
```

(`.delay()` / `.repeat*()` are builders on `Transition`; `Spring`/`Tween`/`KeyframesTiming`/`Inertia` all convert via `From`/`.into()`.) Repeat applies to tweens and keyframes; springs and inertia are physical and ignore it.

### Springs

```rust
Spring::default()  // (170, 26) — react-spring "default"
Spring::gentle()   // (120, 14)
Spring::wobbly()   // (180, 12)
Spring::stiff()    // (310, 26)
Spring::slow()     // (280, 60)

Spring::new(stiffness, damping)
    .mass(1.5)
    .rest(0.001, 0.001)          // explicit rest thresholds (e.g. 0..1 color channels)

// Duration-parameterized springs (Motion-style), Newton-inverted to physics:
Spring::from_duration(0.5)                  // perceptual duration, no bounce
Spring::from_duration_bounce(0.5, 0.3)      // bounce 0..1
Spring::from_visual_duration(0.5, 0.3)      // duration = time to first target crossing
```

Rest thresholds default to *adaptive*: tight for short travels (like color channels), loose for pixel-scale travels.

### Inertia (fling / momentum)

```rust
Inertia::new()
    .bounds(0.0, 340.0)                     // optional min/max with spring bounce-back
    .modify_target(|t| (t / 80.0).round() * 80.0)  // e.g. snap to a grid
```

Inertia animates from the *current velocity* (exponential decay toward a projected target); out-of-bounds targets hand off to a boundary spring. Pair it with `DragTracker` for drag-release flings.

### `presence` / `presence_group` — enter/exit animation

Single element:

```rust
presence("toast", visible, enter_value, exit_value, transition, |value| render(value))
```

When `visible` turns `false` the element keeps rendering while it animates toward `exit_value`; only after settling does it disappear. Flipping `visible` back during the exit reverses with preserved velocity.

Keyed group (Framer's `<AnimatePresence>` over a list):

```rust
presence_group::<f32>("toasts")
    .enter(1.0)
    .exit(0.0)
    .transition(Spring::default())
    .mode(PresenceMode::Sync)        // Sync: exits animate alongside entries
                                     // Wait: entries wait for exits to finish
    .child(("toast", item.id), move |opacity| render(item, opacity))
    // ... one .child per currently-visible item ...
    .on_exit_complete(|key, window, cx| { /* item fully gone */ })
```

Removed children keep animating out — the group caches their render closures until they settle.

### `MotionValue<T>` — a free-standing animated value

```rust
let progress = MotionValue::new(0.0f32, Spring::default().into());
progress.set_target(0.8);
let current = progress.get(window, cx);   // advances; schedules a frame while unsettled

progress.jump(0.0);                       // teleport, no animation
progress.flick(velocity);                 // start an inertia fling from a velocity
progress.set_target_with_velocity(t, v);  // retarget with injected velocity
progress.get_velocity();                  // current velocity
let scaled = progress.map(|p| p * 340.0); // derived read-only value
```

Store it in your `Entity`; clone it into closures (clones share state).

### `DragTracker` — drag with momentum handoff

```rust
// in on_mouse_down:   tracker.begin(event.position);
// in on_mouse_move:   let delta = tracker.update(event.position); // apply to your value
// in on_mouse_up:     let velocity = tracker.end();               // px/sec over last 30ms
//                     motion_value.flick(velocity.x);
```

## Framer Motion mapping

| Framer Motion | gpui-motion |
|---|---|
| `<motion.div animate={{ x, background }} />` | `div().with_motion(id, (x, color), t, ...)` |
| `animate={{ x: [0, 100, 0] }}` (keyframes) | `with_motion(id, [v0, v1, v2], KeyframesTiming::new(d), ...)` |
| `transition={{ type: "spring", stiffness, damping, mass }}` | `Spring::new(stiffness, damping).mass(m)` |
| `transition={{ type: "spring", duration, bounce }}` | `Spring::from_duration_bounce(duration, bounce)` |
| `transition={{ duration, ease }}` | `Tween::new(duration).easing(...)` |
| `transition={{ delay, repeat, repeatType }}` | `.delay(s)` / `.repeat_times(n, kind)` / `.repeat_forever(kind)` |
| `transition={{ x: {...}, background: {...} }}` (per-property) | tuple of transitions: `(Spring::stiff(), Tween::new(0.25))` |
| `transition={{ type: "inertia", ... }}` | `Inertia::new().bounds(min, max).modify_target(f)` |
| `initial={{ ... }}` | `.initial(value)` |
| `whileHover` / `whileTap` | `.while_hover(value)` / `.while_press(value)` |
| `onAnimationComplete` | `.on_settle(...)` |
| `<AnimatePresence>` (single child) | `presence(id, visible, enter, exit, t, render)` |
| `<AnimatePresence mode="sync\|wait">` (list) | `presence_group(id).mode(Sync\|Wait).child(key, render)` |
| `onExitComplete` | `.on_exit_complete(...)` |
| `useMotionValue` / `useSpring` | `MotionValue<T>` |
| `useTransform` | `MotionValue::map` |
| `useVelocity` | `MotionValue::get_velocity` |
| drag + `dragTransition` | `DragTracker` + `MotionValue::flick` |
| `useReducedMotion` | automatic: `cx.reduce_motion()` ⇒ snap to target |

## Version notes

This crate pins gpui as a **git dependency on Zed's main branch** (the exact commit is resolved by your `Cargo.lock`):

```toml
gpui = { git = "https://github.com/zed-industries/zed" }
```

- Git dependencies only unify if downstream crates use the same source, so your app should depend on gpui the same way (a `[patch.crates-io]` entry works if some of your dependencies use the crates.io `gpui`).
- The element-layer API surface used here (`Element` trait with `inspector_id`, `Window::with_element_state`, `request_animation_frame`, `insert_hitbox`, `on_mouse_event`) is identical in crates.io `gpui 0.2.2`; the one thing 0.2.2 lacks is `App::reduce_motion` (added to gpui in mid-2026), so building against 0.2.2 requires removing that call.
- The engine (`--no-default-features`) has **zero dependencies** and compiles and tests without gpui.
- The demo additionally uses `gpui_platform` (windowing backends) — see `examples/demo`.

## Known limitations

- **No CSS-style transforms.** gpui's `div` has no `transform` property. Express translation via `left`/`top` (with absolute positioning) or margins, and scale via `w`/`h`. This is a framework fact, not a bug in this crate.
- **`ElementId` must be stable across frames.** Element state is keyed by id; an unstable id means the state is lost every frame and the animation restarts from scratch. In lists, key by your data (`("row", item.id)`), never by loop index.
- **At most 8 channels per animated value** (`MAX_CHANNELS`) and **at most 8 keyframes** (`MAX_KEYFRAMES`). Split larger values across multiple `with_motion` wrappers.
- **`Hsla` is interpolated by converting through `Rgba`** to avoid hue-wheel long-arc artifacts (red → blue passing through green). Alpha is interpolated as-is.
- **Repeat applies to tweens/keyframes only.** Springs and inertia are physical simulations; repeating them makes no sense and the flag is ignored.
- **Reduced motion:** when the OS/user setting is on (`cx.reduce_motion()`), every animation snaps directly to its target. This is an accessibility requirement, not an option.

## Demo

```sh
cargo run -p demo
```

Scenes: spring panel (`(Pixels, Rgba)` tuple), presence toast, `MotionValue` progress bar, keyframe pulse, repeat modes, per-property transitions, hover/press gestures, drag-with-inertia, and a keyed `presence_group` list in both Sync and Wait modes.

## Design decisions (for contributors)

- **Element state, not a global registry** — state is created and reclaimed with the element's lifecycle; the cost is the stable-`ElementId` requirement above.
- **The engine only knows `f32` channels** — `Animatable` does the (de)composition; colors go through linear `Rgba` space.
- **Transitions are caller-supplied per frame** — never persisted, so per-direction parameters are free.
- **Closed-form springs** — exact solutions per damping branch instead of numeric integration; robust to any frame gap by construction.

## Non-goals for v0.2 (design keeps the door open)

Variants/`staggerChildren` orchestration, FLIP layout animation, `popLayout` presence mode, scroll-linked values, and string interpolation. The velocity-preserving engine already supports the interruption semantics these need.
