use gtk::prelude::*;
use relm4::{gtk, prelude::*};

use super::key_seq::KeySeqInput;
use crate::config_file::RemapConfig;

#[derive(Debug)]
pub struct RemapItem {
    pub input_seq: Controller<KeySeqInput>,
    pub output_seq: Controller<KeySeqInput>,
}

#[derive(Debug)]
pub enum RemapItemOutput {
    Delete(DynamicIndex),
}

#[relm4::factory(pub)]
impl FactoryComponent for RemapItem {
    type Init = RemapConfig;
    type Input = ();
    type Output = RemapItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Frame {
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 12,
                set_margin_all: 12,

                gtk::Grid {
                    set_row_spacing: 6,
                    set_column_spacing: 6,

                    attach[0,0,1,1] = &gtk::Label {
                        set_label: "Input:",
                        set_halign: gtk::Align::Start,
                    },

                    attach[1,0,1,1] = self.input_seq.widget(),

                    attach[0,1,1,1] = &gtk::Label {
                        set_label: "Output:",
                        set_halign: gtk::Align::Start,
                    },

                    attach[1,1,1,1] = self.output_seq.widget(),
                },

                gtk::Button::from_icon_name("edit-delete-symbolic") {
                    connect_clicked[sender, index] => move |_| {
                        sender.output(RemapItemOutput::Delete(index.clone())).unwrap();
                    }
                }
            }
        }
    }

    fn init_model(init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        let input_seq = KeySeqInput::builder().launch(init.input).detach();
        let output_seq = KeySeqInput::builder().launch(init.output).detach();
        Self {
            input_seq,
            output_seq,
        }
    }
}
