use std::sync::OnceLock;

pub use evdev_rs::enums::EV_KEY as KeyCode;
use evdev_rs::enums::EventCode;

pub fn list_keycodes() -> &'static [KeyCode] {
    static KEYS_ONCE_CELL: OnceLock<Vec<KeyCode>> = OnceLock::new();

    KEYS_ONCE_CELL
        .get_or_init(|| {
            let mut keys: Vec<KeyCode> = EventCode::EV_KEY(KeyCode::KEY_RESERVED)
                .iter()
                .filter_map(|code| match code {
                    EventCode::EV_KEY(k) => Some(k),
                    _ => None,
                })
                .collect();

            keys.sort_by_cached_key(|k| format!("{}", EventCode::EV_KEY(*k)));
            keys
        })
        .as_slice()
}

pub fn list_keynames_iter() -> impl Iterator<Item = String> {
    list_keycodes()
        .iter()
        .map(|k| format!("{}", EventCode::EV_KEY(*k)))
}
