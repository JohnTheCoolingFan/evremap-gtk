use gtk::prelude::*;
use relm4::prelude::*;

use super::key_seq::KeySeqInput;
use crate::{
    components::key_seq::new_dropdown_property_expr,
    config_file::DualRoleConfig,
    evdev_utils::{KeyCode, list_keycodes, list_keynames_iter},
};

#[derive(Debug)]
pub struct DualRoleMapItem {
    pub key: KeyCode,
    pub hold_seq: Controller<KeySeqInput>,
    pub tap_seq: Controller<KeySeqInput>,
}

#[derive(Debug)]
pub enum DualRoleMapItemMsg {
    SelectTriggerKey(KeyCode),
}

#[derive(Debug)]
pub enum DualRoleMapItemOutput {
    Delete(DynamicIndex),
}

#[relm4::factory(pub)]
impl FactoryComponent for DualRoleMapItem {
    type Init = DualRoleConfig;
    type Input = DualRoleMapItemMsg;
    type Output = DualRoleMapItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Frame {
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 12,
                set_margin_all: 12,


                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 6,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,

                        gtk::Label {
                            set_text: "Trigger key:",
                        },

                        gtk::DropDown::new(
                            Some(gtk::StringList::from_iter(list_keynames_iter())),
                            Some(new_dropdown_property_expr())
                        ) {
                            set_enable_search: true,
                            set_search_match_mode: gtk::StringFilterMatchMode::Substring,
                            connect_selected_notify[sender] => move |dd| {
                                let idx = dd.selected();
                                if idx != gtk::INVALID_LIST_POSITION {
                                    sender.input(DualRoleMapItemMsg::SelectTriggerKey(list_keycodes()[(idx) as usize]));
                                }
                            }
                        }
                    },

                    gtk::Grid {
                        set_row_spacing: 6,
                        set_column_spacing: 6,


                        attach[0,1,1,1] = &gtk::Label {
                            set_label: "Hold:",
                            set_halign: gtk::Align::Start,
                        },

                        attach[1,1,1,1] = self.hold_seq.widget(),

                        attach[0,2,1,1] = &gtk::Label {
                            set_label: "Tap:",
                            set_halign: gtk::Align::Start,
                        },

                        attach[1,2,1,1] = self.tap_seq.widget(),
                    },
                },

                gtk::Button::from_icon_name("edit-delete-symbolic") {
                    connect_clicked[sender, index] => move |_| {
                        sender.output(DualRoleMapItemOutput::Delete(index.clone())).unwrap();
                    }
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        let hold_seq = KeySeqInput::builder().launch(init.hold).detach();
        let tap_seq = KeySeqInput::builder().launch(init.tap).detach();
        Self {
            key: init.input,
            hold_seq,
            tap_seq,
        }
    }

    fn update(&mut self, message: Self::Input, _sender: FactorySender<Self>) {
        match message {
            DualRoleMapItemMsg::SelectTriggerKey(k) => {
                self.key = k;
            }
        }
    }
}
