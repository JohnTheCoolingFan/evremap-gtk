use gtk::prelude::*;
use relm4::prelude::*;

use crate::deviceinfo::DeviceInfo;

#[derive(Debug, Clone)]
pub struct DeviceDisplay {
    device: DeviceInfo,
    hidden: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum DeviceDisplayMsg {
    ShowHidden,
    HideUseless,
}

#[derive(Debug)]
pub enum DeviceDisplayOutput {
    SetDevice(DeviceInfo),
    UseDeviceInLogger(DeviceInfo),
}

#[relm4::factory(pub)]
impl FactoryComponent for DeviceDisplay {
    type Init = DeviceInfo;
    type Input = DeviceDisplayMsg;
    type Output = DeviceDisplayOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        #[root]
        gtk::Frame {
            #[watch]
            set_visible: !self.hidden || self.device.supports_remap,
            set_hexpand: true,

            gtk::Grid {
                set_margin_all: 12,
                set_row_spacing: 12,
                set_column_spacing: 12,

                attach[0,0,1,1] = &gtk::Label {
                    set_label: "Device name:",
                    set_halign: gtk::Align::Start,
                },

                attach[1,0,1,1] = &gtk::Label {
                    set_label: &self.device.name,
                    set_selectable: true,
                    set_halign: gtk::Align::Start,
                    set_hexpand: true
                },

                attach[0,1,1,1] = &gtk::Label {
                    set_label: "Device phys:",
                    set_halign: gtk::Align::Start,
                },

                attach[1,1,1,1] = &gtk::Label {
                    set_label: self.device.phys.as_ref().map_or("(Missing)", |v| v),
                    set_selectable: self.device.phys.is_some(),
                    set_halign: gtk::Align::Start,
                    set_hexpand: true,
                },

                attach[0,2,1,1] = &gtk::Label {
                    set_label: "Device path:",
                    set_halign: gtk::Align::Start,
                },

                attach[1,2,1,1] = &gtk::Label {
                    set_label: &format!("{}", self.device.path.display()),
                    set_selectable: self.device.phys.is_some(),
                    set_halign: gtk::Align::Start,
                    set_hexpand: true,
                },

                attach[2,0,1,3] = &gtk::Button::from_icon_name("object-select-symbolic") {
                    set_tooltip_text: Some("Use this device"),
                    connect_clicked[sender, device_cl = self.device.clone()] => move |_| {
                        sender.output(DeviceDisplayOutput::SetDevice(device_cl.clone())).unwrap();
                    }
                },

                attach[3,0,1,3] = &gtk::Button::from_icon_name("view-paged-symbolic") {
                    set_tooltip_text: Some("See device events"),
                    connect_clicked[sender, device_cl = self.device.clone()] => move |_| {
                        sender.output(DeviceDisplayOutput::UseDeviceInLogger(device_cl.clone())).unwrap();
                    }
                },
            }
        }
    }

    fn init_model(init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        Self {
            device: init,
            hidden: true,
        }
    }

    fn update(&mut self, message: Self::Input, _sender: FactorySender<Self>) {
        match message {
            DeviceDisplayMsg::ShowHidden => self.hidden = false,
            DeviceDisplayMsg::HideUseless => self.hidden = true,
        }
    }
}
