use evdev_rs::enums::EventCode;
use gtk::prelude::*;
use relm4::prelude::*;

use crate::evdev_utils::{KeyCode, list_keycodes, list_keynames_iter};

#[derive(Debug)]
pub struct KeySeqInput {
    pub sequence: Vec<KeyCode>,
}

#[derive(Debug)]
pub enum KeySeqInputMsg {
    SetSequence(Vec<KeyCode>),
    AddKey(KeyCode),
    PopKey,
    ClearKeys,
}

impl KeySeqInput {
    fn key_seq_to_string(&self) -> String {
        let mut key_iter = self.sequence.iter();
        let mut buf = String::new();
        if let Some(key) = key_iter.next() {
            buf.push_str(&format!("{}", EventCode::EV_KEY(*key)));
            for key in key_iter {
                buf.push('+');
                buf.push_str(&format!("{}", EventCode::EV_KEY(*key)));
            }
        }
        buf
    }
}

pub fn new_dropdown_property_expr() -> gtk::PropertyExpression {
    gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    )
}

#[relm4::component(pub)]
impl SimpleComponent for KeySeqInput {
    type Init = Vec<KeyCode>;
    type Input = KeySeqInputMsg;
    type Output = ();

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 6,

            gtk::Entry {
                #[watch]
                set_text: &model.key_seq_to_string(),
                set_hexpand: true,
                set_editable: false,
            },

            gtk::Button::from_icon_name("edit-redo-symbolic-rtl") {
                connect_clicked => KeySeqInputMsg::PopKey
            },

            gtk::DropDown::new(
                Some(gtk::StringList::from_iter(["Add key...".to_owned()].into_iter().chain(list_keynames_iter()))),
                Some(new_dropdown_property_expr())
            ) {
                set_enable_search: true,
                set_search_match_mode: gtk::StringFilterMatchMode::Substring,
                connect_selected_notify[sender] => move |dd| {
                    let idx = dd.selected();
                    if idx != gtk::INVALID_LIST_POSITION && idx != 0 {
                        sender.input(KeySeqInputMsg::AddKey(list_keycodes()[(idx-1) as usize]));
                        dd.set_selected(0);
                    }
                }
            },

            gtk::Button::from_icon_name("edit-clear-symbolic") {
                connect_clicked => KeySeqInputMsg::ClearKeys,
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self { sequence: init };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            KeySeqInputMsg::SetSequence(seq) => {
                self.sequence = seq;
            }
            KeySeqInputMsg::AddKey(k) => {
                self.sequence.push(k);
            }
            KeySeqInputMsg::PopKey => {
                self.sequence.pop();
            }
            KeySeqInputMsg::ClearKeys => {
                self.sequence.clear();
            }
        }
    }
}
