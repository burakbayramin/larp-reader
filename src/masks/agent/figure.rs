//! The larper figure beside the welcome box, and how it moves.

use std::time::Duration;

/// The welcome figure, a larper: a role-player in a homemade cardboard helmet
/// with a foam sword, posing. 10 cells wide; these are the rows as the header
/// draws them, sword up, standing proud.
pub const SPRITE: [&str; 4] = [" ▄█████▄ │", " █ ▀ ▀ █ │", " ▐█████▌▄┼", "  ▟▛ ▜▙  █"];
/// The flourish: sword lifted forward (the top two rows), then swung down.
pub(super) const SPRITE_RAISED: [&str; 2] = [" ▄█████▄ ╱", " █ ▀ ▀ █╱ "];
pub(super) const SPRITE_SWUNG: [&str; 2] = [" ▄█████▄  ", " █ ▀ ▀ █╲ "];
/// The foam blade droops (the top row); it straightens when the window ends.
pub(super) const SPRITE_DROOP: &str = " ▄█████▄ ╮";
/// Marching in place (the bottom row), left foot up, then right foot up.
pub(super) const SPRITE_MARCH: [&str; 2] = ["  ▗▘ ▜▙  █", "  ▟▛ ▝▖  █"];
/// How often the figure does something showy, and for how long. Every third
/// time the foam sword droops instead of flourishing.
pub const SHOW_EVERY: Duration = Duration::from_millis(4000);
pub const SHOW_FOR: Duration = Duration::from_millis(1200);
/// How long each step lasts while text streams.
pub const STEP_FOR: Duration = Duration::from_millis(360);

/// The figure's rows for `elapsed` into the session: (as drawn, as shown now).
/// It holds its pose; at the end of every `SHOW_EVERY` it raises and swings
/// the sword (or lets it droop, every third time). While text streams it
/// marches in place with the sword up.
pub fn sprite_frame(elapsed: Duration, active: bool) -> Vec<(&'static str, &'static str)> {
    let ms = elapsed.as_millis();
    let every = SHOW_EVERY.as_millis();
    let into = ms % every;
    let from = (SHOW_EVERY - SHOW_FOR).as_millis();
    let mut now = SPRITE;
    if !active && into >= from {
        if (ms / every) % 3 == 2 {
            now[0] = SPRITE_DROOP;
        } else {
            let pair = if into - from < SHOW_FOR.as_millis() / 2 {
                SPRITE_RAISED
            } else {
                SPRITE_SWUNG
            };
            now[..2].copy_from_slice(&pair);
        }
    }
    if active {
        now[3] = SPRITE_MARCH[(ms / STEP_FOR.as_millis()) as usize % 2];
    }
    SPRITE.iter().copied().zip(now).collect()
}
