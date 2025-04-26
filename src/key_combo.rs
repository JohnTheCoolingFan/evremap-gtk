use crate::evdev_utils::KeyCode;

// Same as in evremap
fn is_modifier(key: &KeyCode) -> bool {
    matches!(
        key,
        KeyCode::KEY_FN
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
            | KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
    )
}

macro_rules! modkey_translate {
    ($keycode_val:ident, $keycode_t:ty, [$($keycode_name:ident),+]) => {
        match $keycode_val {
            $(
                <$keycode_t>::$keycode_name => Some(Self::$keycode_name),
            )+
            _ => None
        }
    }
}

const MODIFIERS: [KeyCode; 9] = [
    KeyCode::KEY_FN,
    KeyCode::KEY_LEFTALT,
    KeyCode::KEY_RIGHTALT,
    KeyCode::KEY_LEFTMETA,
    KeyCode::KEY_RIGHTMETA,
    KeyCode::KEY_LEFTCTRL,
    KeyCode::KEY_RIGHTCTRL,
    KeyCode::KEY_LEFTSHIFT,
    KeyCode::KEY_RIGHTSHIFT,
];

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
struct ModifierKeysMask(u16);

impl ModifierKeysMask {
    const KEY_FN: Self = Self(1);
    const KEY_LEFTALT: Self = Self(1 << 1);
    const KEY_RIGHTALT: Self = Self(1 << 2);
    const KEY_LEFTMETA: Self = Self(1 << 3);
    const KEY_RIGHTMETA: Self = Self(1 << 4);
    const KEY_LEFTCTRL: Self = Self(1 << 5);
    const KEY_RIGHTCTRL: Self = Self(1 << 6);
    const KEY_LEFTSHIFT: Self = Self(1 << 7);
    const KEY_RIGHTSHIFT: Self = Self(1 << 8);

    fn from_keycode(key: KeyCode) -> Option<Self> {
        modkey_translate!(
            key,
            KeyCode,
            [
                KEY_FN,
                KEY_LEFTALT,
                KEY_RIGHTALT,
                KEY_LEFTMETA,
                KEY_RIGHTMETA,
                KEY_LEFTCTRL,
                KEY_RIGHTCTRL,
                KEY_LEFTSHIFT,
                KEY_RIGHTSHIFT
            ]
        )
    }

    fn add(&mut self, key: KeyCode) {
        if let Some(keymask) = Self::from_keycode(key) {
            self.0 |= keymask.0
        }
    }

    fn remove(&mut self, key: KeyCode) {
        if let Some(keymask) = Self::from_keycode(key) {
            self.0 &= !keymask.0
        }
    }

    fn contains(&self, key: KeyCode) -> bool {
        if let Some(keymask) = Self::from_keycode(key) {
            (self.0 & keymask.0) != 0
        } else {
            false
        }
    }

    fn into_iter(self) -> impl Iterator<Item = KeyCode> {
        MODIFIERS.into_iter().filter(move |k| self.contains(*k))
    }

    fn pop(&mut self) -> Option<KeyCode> {
        if self.0 == 0 {
            None
        } else {
            for key in MODIFIERS.into_iter().rev() {
                if self.contains(key) {
                    self.remove(key);
                    return Some(key);
                }
            }
            // Should be unreachable
            None
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeyCombination {
    modifiers: ModifierKeysMask,
    keys: Vec<KeyCode>,
}

impl KeyCombination {
    pub fn clear(&mut self) {
        self.modifiers = ModifierKeysMask::default();
        self.keys.clear();
    }

    pub fn push(&mut self, key: KeyCode) {
        if is_modifier(&key) {
            self.modifiers.add(key)
        } else if !self.keys.contains(&key) {
            self.keys.push(key);
        }
    }

    pub fn pop(&mut self) -> Option<KeyCode> {
        if !self.keys.is_empty() {
            self.keys.pop()
        } else {
            self.modifiers.pop()
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = KeyCode> {
        self.modifiers.into_iter().chain(self.keys.iter().copied())
    }

    pub fn to_keys(&self) -> Vec<KeyCode> {
        self.modifiers
            .into_iter()
            .chain(self.keys.iter().copied())
            .collect()
    }
}

impl From<KeyCombination> for Vec<KeyCode> {
    fn from(value: KeyCombination) -> Self {
        value.modifiers.into_iter().chain(value.keys).collect()
    }
}

impl FromIterator<KeyCode> for KeyCombination {
    fn from_iter<T: IntoIterator<Item = KeyCode>>(iter: T) -> Self {
        let mut res = Self::default();
        for key in iter {
            if is_modifier(&key) {
                res.modifiers.add(key);
            } else {
                res.keys.push(key);
            }
        }
        res
    }
}

impl From<Vec<KeyCode>> for KeyCombination {
    fn from(value: Vec<KeyCode>) -> Self {
        Self::from_iter(value)
    }
}
