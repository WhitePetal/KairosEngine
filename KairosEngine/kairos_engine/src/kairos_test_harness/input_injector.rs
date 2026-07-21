use crate::inputs::{Input, InputEngine};
use crate::kairos_test_harness::types::StepResult;

/// Translate a TOML `input` step into engine input injection.
///
/// Supported devices:
/// - `keyboard`: `press`/`release` events with a `key` arg (W, A, S, D)
/// - `mouse`: `click` event with `button` (Left, Right), or `move` event with x/y coords
pub fn inject(args: &toml::Value, input_engine: &mut InputEngine) -> StepResult {
    let device = match args.get("device").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return StepResult::err("missing 'device' field in input step"),
    };

    let event = match args.get("event").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return StepResult::err("missing 'event' field in input step"),
    };

    match device {
        "keyboard" => inject_keyboard(event, args, input_engine),
        "mouse" => inject_mouse(event, args, input_engine),
        other => StepResult::err(format!("unknown device: '{other}'")),
    }
}

fn inject_keyboard(event: &str, args: &toml::Value, input_engine: &mut InputEngine) -> StepResult {
    let key = match args.get("key").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return StepResult::err("missing 'key' field for keyboard input"),
    };

    let input = match match_key_to_input(key) {
        Ok(input) => input,
        Err(e) => return e,
    };
    let pressed = match event {
        "press" => true,
        "release" => false,
        other => return StepResult::err(format!("unknown keyboard event: '{other}'")),
    };

    input_engine.inject_input(input, pressed);
    StepResult::ok()
}

fn inject_mouse(event: &str, args: &toml::Value, input_engine: &mut InputEngine) -> StepResult {
    match event {
        "click" => {
            let button = match args.get("button").and_then(|v| v.as_str()) {
                Some(b) => b,
                None => return StepResult::err("missing 'button' field for mouse click"),
            };

            let input = match button {
                "Left" => Input::MouseLClick,
                "Right" => Input::MouseRClick,
                other => {
                    return StepResult::err(format!("unknown mouse button: '{other}'"));
                }
            };

            // Mouse click: press then release
            input_engine.inject_input(input, true);
            input_engine.inject_input(input, false);
            StepResult::ok()
        }
        "move" => {
            let x = args.get("x").and_then(|v| v.as_float()).unwrap_or(0.0) as f32;
            let y = args.get("y").and_then(|v| v.as_float()).unwrap_or(0.0) as f32;

            input_engine.inject_mouse_position(crate::math::float2::new(x, y));
            StepResult::ok()
        }
        other => StepResult::err(format!("unknown mouse event: '{other}'")),
    }
}

/// Map a key name string to the corresponding `Input` enum variant.
fn match_key_to_input(key: &str) -> Result<Input, StepResult> {
    match key {
        "W" => Ok(Input::W),
        "A" => Ok(Input::A),
        "S" => Ok(Input::S),
        "D" => Ok(Input::D),
        other => Err(StepResult::err(format!("unknown key: '{other}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> InputEngine {
        InputEngine::new()
    }

    #[test]
    fn inject_keyboard_press() {
        let args: toml::Value =
            toml::from_str("device = 'keyboard'\nevent = 'press'\nkey = 'W'").unwrap();
        let mut engine = make_engine();
        let result = inject(&args, &mut engine);
        assert!(result.ok);
    }

    #[test]
    fn inject_keyboard_release() {
        let args: toml::Value =
            toml::from_str("device = 'keyboard'\nevent = 'release'\nkey = 'D'").unwrap();
        let mut engine = make_engine();
        let result = inject(&args, &mut engine);
        assert!(result.ok);
    }

    #[test]
    fn inject_keyboard_unknown_key_fails() {
        let args: toml::Value =
            toml::from_str("device = 'keyboard'\nevent = 'press'\nkey = 'Z'").unwrap();
        let mut engine = make_engine();
        let result = inject(&args, &mut engine);
        assert!(!result.ok);
        assert!(result.message.contains("unknown key"));
    }

    #[test]
    fn inject_mouse_click_left() {
        let args: toml::Value =
            toml::from_str("device = 'mouse'\nevent = 'click'\nbutton = 'Left'").unwrap();
        let mut engine = make_engine();
        let result = inject(&args, &mut engine);
        assert!(result.ok);
    }

    #[test]
    fn inject_mouse_move() {
        let args: toml::Value =
            toml::from_str("device = 'mouse'\nevent = 'move'\nx = 100.0\ny = 200.0").unwrap();
        let mut engine = make_engine();
        let result = inject(&args, &mut engine);
        assert!(result.ok);
    }

    #[test]
    fn inject_missing_device_fails() {
        let args: toml::Value = toml::from_str("event = 'press'").unwrap();
        let mut engine = make_engine();
        let result = inject(&args, &mut engine);
        assert!(!result.ok);
    }
}
