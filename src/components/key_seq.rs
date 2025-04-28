use evdev_rs::enums::EventCode;
use gtk::prelude::*;
use relm4::prelude::*;

use crate::{
    evdev_utils::{KeyCode, list_keycodes, list_keynames_iter},
    key_combo::KeyCombination,
};

#[derive(Debug)]
struct KeyButton {
    key: KeyCode,
}

#[derive(Debug)]
enum KeyButtonOutput {
    Remove(KeyCode),
}

#[relm4::factory]
impl FactoryComponent for KeyButton {
    type Init = KeyCode;
    type Input = ();
    type Output = KeyButtonOutput;
    type ParentWidget = gtk::Box;
    type CommandOutput = ();

    view! {
        #[root]
        gtk::Button {
            set_label: &format!("{}", EventCode::EV_KEY(self.key)),
            set_tooltip_text: Some("Click to remove the key"),
            connect_clicked[sender, keycode = self.key] => move |_| {
                sender.output(KeyButtonOutput::Remove(keycode)).unwrap()
            }
        }
    }

    fn init_model(init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        Self { key: init }
    }
}

#[derive(Debug)]
pub struct KeySeqInput {
    pub sequence: KeyCombination,
    keys_factory: FactoryVecDeque<KeyButton>,
}

#[derive(Debug)]
pub enum KeySeqInputMsg {
    AddKey(KeyCode),
    ClearKeys,
    RemoveKey(KeyCode),
}

impl KeySeqInput {
    fn keys_factory_update(&mut self) {
        let mut kfac = self.keys_factory.guard();
        kfac.clear();
        for key in self.sequence.iter() {
            kfac.push_back(key);
        }
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

            gtk::ScrolledWindow {
                set_policy: (gtk::PolicyType::Automatic, gtk::PolicyType::Never),
                #[local_ref]
                keys_factory_box -> gtk::Box {
                    set_hexpand: true,
                    set_spacing: 6,
                },
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
        let keys =
            FactoryVecDeque::builder()
                .launch_default()
                .forward(sender.input_sender(), |msg| match msg {
                    KeyButtonOutput::Remove(key) => KeySeqInputMsg::RemoveKey(key),
                });

        let model = Self {
            sequence: init.into(),
            keys_factory: keys,
        };

        let keys_factory_box = model.keys_factory.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            KeySeqInputMsg::AddKey(k) => {
                self.sequence.push(k);
            }
            KeySeqInputMsg::ClearKeys => {
                self.sequence.clear();
            }
            KeySeqInputMsg::RemoveKey(key) => {
                self.sequence.remove(key);
            }
        }
        self.keys_factory_update()
    }
}
