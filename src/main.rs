use std::path::PathBuf;

use components::{
    device_browser::{DeviceDisplay, DeviceDisplayOutput},
    dual_role::{DualRoleMapItem, DualRoleMapItemOutput},
    event_logger::{EventLogger, EventLoggerMsg},
    key_seq::KeySeqInputMsg,
    remap::{RemapItem, RemapItemOutput},
};
use config_file::{ConfigFile, DualRoleConfig, RemapConfig};
use deviceinfo::DeviceInfo;
use gtk::{self, prelude::*};
use relm4::{factory::FactoryVecDequeGuard, prelude::*};
use relm4_components::{
    open_dialog::{OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings},
    save_dialog::{SaveDialog, SaveDialogMsg, SaveDialogResponse, SaveDialogSettings},
};

mod components;
mod evdev_utils;

mod config_file;
mod deviceinfo;

const APP_ID: &str = "ru.jtcf.evremap_gtk";

fn main() {
    let app = RelmApp::new(APP_ID);
    relm4::set_global_css(
        ".device-list-refresh-button {
            border-bottom-left-radius: 0px;
            border-bottom-right-radius: 0px;
            border-top-left-radius: 0px;
            border-top-right-radius: 0px;
        }",
    );
    app.run::<AppModel>(());
}

#[derive(Debug, Default)]
struct ConfigFileGtkBuf {
    name: gtk::EntryBuffer,
    phys: gtk::EntryBuffer,
}

impl ConfigFileGtkBuf {
    fn update_from_file(&self, file: &ConfigFile) {
        if let Some(name) = &file.device_name {
            self.name.set_text(name);
        } else {
            self.name.delete_text(0, None);
        }
        if let Some(phys) = &file.phys {
            self.phys.set_text(phys);
        } else {
            self.phys.delete_text(0, None);
        }
    }

    fn to_config_file(
        &self,
        remap: Vec<RemapConfig>,
        dual_role: Vec<DualRoleConfig>,
    ) -> ConfigFile {
        ConfigFile {
            device_name: Some(self.name.text())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            phys: Some(self.phys.text())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            dual_role,
            remap,
        }
    }
}

#[derive(Debug)]
enum CommandMsg {
    UpdateDeviceList(Vec<DeviceInfo>),
}

#[derive(Debug)]
struct AppModel {
    config: ConfigFileGtkBuf,
    remaps: FactoryVecDeque<RemapItem>,
    dual_role_remaps: FactoryVecDeque<DualRoleMapItem>,
    open_dialog: Controller<OpenDialog>,
    save_dialog: Controller<SaveDialog>,
    device_browser: FactoryVecDeque<DeviceDisplay>,
    event_logger: Controller<EventLogger>,
}

#[derive(Debug)]
enum AppMsg {
    Ignore,
    SaveRequest,
    SaveResponse(PathBuf),
    OpenRequest,
    OpenResponse(PathBuf),
    AddRemap,
    DeleteRemap(DynamicIndex),
    AddDualRoleRemap,
    DeleteDualRoleRemap(DynamicIndex),
    SetDevice(DeviceInfo),
    RefreshDevices,
    SetLoggerDevice(DeviceInfo),
}

#[relm4::component]
impl Component for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CommandMsg;

    view! {
        gtk::Window {
            set_title: Some("evremap config editor"),
            set_default_size: (600, 400),

            #[wrap(Some)]
            set_titlebar = &gtk::HeaderBar {
                pack_start = &gtk::Button {
                    set_label: "Open",
                    connect_clicked => AppMsg::OpenRequest,
                },
                pack_end = &gtk::Button {
                    set_label: "Save As",
                    connect_clicked => AppMsg::SaveRequest,

                    #[watch]
                    set_sensitive: !device_name_entry.text().is_empty()
                },
                #[wrap(Some)]
                set_title_widget = &gtk::StackSwitcher {
                    set_stack: Some(&contents_stack)
                },
            },

            #[name(contents_stack)]
            gtk::Stack {
                add_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,
                    set_margin_all: 12,

                    #[name = "device_name_entry"]
                    gtk::Entry {
                        set_placeholder_text: Some("Device name (required)"),
                        set_buffer: &model.config.name,
                        connect_changed => AppMsg::Ignore,
                    },
                    gtk::Entry {
                        set_placeholder_text: Some("Device phys (optional)"),
                        set_buffer: &model.config.phys,
                    },

                    gtk::Separator::new(gtk::Orientation::Horizontal),


                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,

                            gtk::Button {
                                set_label: "Add remap",
                                connect_clicked => AppMsg::AddRemap,
                            },

                            #[local_ref]
                            remaps_box -> gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 6,
                            },

                            gtk::Separator::new(gtk::Orientation::Horizontal),

                            gtk::Button {
                                set_label: "Add dual-role remap",
                                connect_clicked => AppMsg::AddDualRoleRemap,
                            },

                            #[local_ref]
                            dual_role_box -> gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_spacing: 6,
                            },
                        }
                    }
                } -> {
                    set_name: "editor",
                    set_title: "Editor"
                },

                add_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    gtk::Button::from_icon_name("view-refresh-symbolic") {
                        set_tooltip_text: Some("Refresh device list"),
                        set_has_frame: false,
                        add_css_class: "device-list-refresh-button",
                        connect_clicked => AppMsg::RefreshDevices,
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true,

                        #[local_ref]
                        device_browser_box -> gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_all: 12,
                            set_spacing: 12,
                        }
                    }
                } -> {
                    set_name: "devbrowser",
                    set_title: "Devices"
                },

                #[local_ref]
                add_child = event_logger_box -> gtk::Box {} -> {
                    set_name: "event_logger",
                    set_title: "Events"
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: relm4::ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        let save_dialog = SaveDialog::builder()
            .transient_for_native(&root)
            .launch(SaveDialogSettings::default())
            .forward(sender.input_sender(), |response| match response {
                SaveDialogResponse::Cancel => AppMsg::Ignore,
                SaveDialogResponse::Accept(path) => AppMsg::SaveResponse(path),
            });

        let open_dialog = OpenDialog::builder()
            .transient_for_native(&root)
            .launch(OpenDialogSettings::default())
            .forward(sender.input_sender(), |response| match response {
                OpenDialogResponse::Cancel => AppMsg::Ignore,
                OpenDialogResponse::Accept(path) => AppMsg::OpenResponse(path),
            });

        let event_logger = EventLogger::builder().launch(None).detach();

        let remaps = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |out| match out {
                RemapItemOutput::Delete(idx) => AppMsg::DeleteRemap(idx),
            });

        let dual_role_remaps = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |out| match out {
                DualRoleMapItemOutput::Delete(idx) => AppMsg::DeleteDualRoleRemap(idx),
            });

        let device_browser = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |out| match out {
                DeviceDisplayOutput::SetDevice(dev) => AppMsg::SetDevice(dev),
                DeviceDisplayOutput::UseDeviceInLogger(dev) => AppMsg::SetLoggerDevice(dev),
            });

        sender.input(AppMsg::RefreshDevices);

        let model = Self {
            config: ConfigFileGtkBuf::default(),
            remaps,
            dual_role_remaps,
            save_dialog,
            open_dialog,
            device_browser,
            event_logger,
        };

        let remaps_box = model.remaps.widget();
        let dual_role_box = model.dual_role_remaps.widget();
        let device_browser_box = model.device_browser.widget();
        let event_logger_box = model.event_logger.widget();
        let widgets = view_output!();

        relm4::ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match message {
            AppMsg::Ignore => {}
            AppMsg::SaveRequest => self.save_dialog.emit(SaveDialogMsg::Save),
            AppMsg::SaveResponse(path) => self.to_config_file().save_to(path).unwrap(),
            AppMsg::OpenRequest => self.open_dialog.emit(OpenDialogMsg::Open),
            AppMsg::OpenResponse(path) => {
                let config = ConfigFile::read_from(path).unwrap();
                self.load(config);
            }
            AppMsg::AddRemap => {
                self.remaps.guard().push_back(RemapConfig::default());
            }
            AppMsg::DeleteRemap(idx) => {
                let index = idx.current_index();
                self.remaps.guard().remove(index);
            }
            AppMsg::AddDualRoleRemap => {
                self.dual_role_remaps
                    .guard()
                    .push_back(DualRoleConfig::default());
            }
            AppMsg::DeleteDualRoleRemap(idx) => {
                let index = idx.current_index();
                self.dual_role_remaps.guard().remove(index);
            }
            AppMsg::SetDevice(dev) => {
                self.config.name.set_text(dev.name);
                if let Some(devphys) = dev.phys {
                    self.config.phys.set_text(devphys);
                }
            }
            AppMsg::RefreshDevices => {
                sender.spawn_oneshot_command(|| {
                    CommandMsg::UpdateDeviceList(DeviceInfo::obtain_device_list().unwrap())
                });
            }
            AppMsg::SetLoggerDevice(dev) => {
                self.event_logger.emit(EventLoggerMsg::SetDevice(dev));
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CommandMsg::UpdateDeviceList(devices) => {
                let mut device_list = self.device_browser.guard();
                device_list.clear();
                for dev in devices {
                    device_list.push_back(dev);
                }
            }
        }
    }
}

impl AppModel {
    fn load(&mut self, config_file: ConfigFile) {
        self.config.update_from_file(&config_file);
        let ConfigFile {
            device_name: _,
            phys: _,
            dual_role: config_dual_role,
            remap: config_remap,
        } = config_file;
        let mut remaps = self.remaps.guard();
        let mut dual_role = self.dual_role_remaps.guard();
        Self::truncate_remaps(&mut remaps, config_remap.len());
        Self::truncate_dual_role(&mut dual_role, config_dual_role.len());
        Self::set_remaps(&mut remaps, config_remap);
        Self::set_dual_role(&mut dual_role, config_dual_role);
    }

    fn truncate_remaps(remaps: &mut FactoryVecDequeGuard<'_, RemapItem>, to_len: usize) {
        if remaps.len() > to_len {
            for _ in 0..(remaps.len() - to_len) {
                remaps.pop_back();
            }
        }
    }

    fn truncate_dual_role(
        dual_role: &mut FactoryVecDequeGuard<'_, DualRoleMapItem>,
        to_len: usize,
    ) {
        if dual_role.len() > to_len {
            for _ in 0..(dual_role.len() - to_len) {
                dual_role.pop_back();
            }
        }
    }

    fn set_remaps(
        remaps: &mut FactoryVecDequeGuard<'_, RemapItem>,
        remaps_iter: impl IntoIterator<Item = RemapConfig>,
    ) {
        for (i, remap_config) in remaps_iter.into_iter().enumerate() {
            if let Some(remap_item) = remaps.get_mut(i) {
                remap_item
                    .input_seq
                    .emit(KeySeqInputMsg::SetSequence(remap_config.input));
                remap_item
                    .output_seq
                    .emit(KeySeqInputMsg::SetSequence(remap_config.output));
            } else {
                remaps.push_back(remap_config);
            }
        }
    }

    fn set_dual_role(
        dual_role: &mut FactoryVecDequeGuard<'_, DualRoleMapItem>,
        dual_role_iter: impl IntoIterator<Item = DualRoleConfig>,
    ) {
        for (i, dual_role_config) in dual_role_iter.into_iter().enumerate() {
            if let Some(dual_role_item) = dual_role.get_mut(i) {
                dual_role_item.key = dual_role_config.input;
                dual_role_item
                    .hold_seq
                    .emit(KeySeqInputMsg::SetSequence(dual_role_config.hold));
                dual_role_item
                    .tap_seq
                    .emit(KeySeqInputMsg::SetSequence(dual_role_config.tap));
            } else {
                dual_role.push_back(dual_role_config);
            }
        }
    }

    fn to_config_file(&self) -> ConfigFile {
        let remaps = self.remaps_extract();
        let dual_remaps = self.dual_remaps_extract();
        self.config.to_config_file(remaps, dual_remaps)
    }

    fn remaps_extract(&self) -> Vec<RemapConfig> {
        self.remaps
            .iter()
            .map(|remap_item| RemapConfig {
                input: remap_item.input_seq.model().sequence.clone(),
                output: remap_item.output_seq.model().sequence.clone(),
            })
            .collect()
    }

    fn dual_remaps_extract(&self) -> Vec<DualRoleConfig> {
        self.dual_role_remaps
            .iter()
            .map(|dual_role| DualRoleConfig {
                input: dual_role.key,
                hold: dual_role.hold_seq.model().sequence.clone(),
                tap: dual_role.tap_seq.model().sequence.clone(),
            })
            .collect()
    }
}
