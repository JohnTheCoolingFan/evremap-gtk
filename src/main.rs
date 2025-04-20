use std::{path::PathBuf, sync::mpsc};

use components::{
    device_browser::{DeviceDisplay, DeviceDisplayOutput},
    dual_role::{DualRoleMapItem, DualRoleMapItemOutput},
    key_seq::KeySeqInputMsg,
    remap::{RemapItem, RemapItemOutput},
};
use config_file::{ConfigFile, DualRoleConfig, RemapConfig};
use deviceinfo::DeviceInfo;
use evdev_rs::enums::EventCode;
use evdev_utils::KeyCode;
#[allow(deprecated)]
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
struct EventLogger {
    device: Option<DeviceLoggerState>,
    text_buf: gtk::TextBuffer,
    is_paused: bool,
}

#[derive(Debug)]
enum BgTaskMsg {
    Stop,
}

#[derive(Debug)]
struct DeviceLoggerState {
    device: DeviceInfo,
    bg_task_sender: mpsc::Sender<BgTaskMsg>,
}

#[derive(Debug)]
enum EventLoggerMsg {
    Pause,
    Resume,
    Clear,
    SetDevice(DeviceInfo),
    ClearDevice,
}

#[derive(Debug)]
enum EventCommandMsg {
    NewEvent(KeyCode, i32),
}

#[relm4::component]
impl Component for EventLogger {
    type Init = Option<DeviceInfo>;
    type Input = EventLoggerMsg;
    type Output = ();
    type CommandOutput = EventCommandMsg;

    view! {
        gtk::Box {
            set_margin_all: 12,
            set_spacing: 6,
            set_orientation: gtk::Orientation::Vertical,

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 6,

                gtk::ToggleButton {
                    set_icon_name: "media-playback-start-symbolic",
                    #[watch]
                    set_active: !model.is_paused,
                    connect_toggled[sender] => move |tb| {
                        if !tb.is_active() {
                            sender.input(EventLoggerMsg::Pause)
                        } else {
                            sender.input(EventLoggerMsg::Resume)
                        }
                    }
                },

                gtk::Button::from_icon_name("edit-clear-symbolic") {
                    set_tooltip_text: Some("Clear event log"),
                    connect_clicked => EventLoggerMsg::Clear,
                },

                gtk::Button::from_icon_name("edit-delete-symbolic") {
                    set_tooltip_text: Some("Clear device"),
                    connect_clicked => EventLoggerMsg::ClearDevice,
                }
            },

            gtk::Frame {
                set_hexpand: true,

                #[wrap(Some)]
                set_child = match &model.device {
                    Some(dev) => {
                        gtk::Grid {
                            set_margin_all: 12,
                            set_row_spacing: 12,
                            set_column_spacing: 12,

                            attach[0,0,1,1] = &gtk::Label {
                                set_label: "Device name:",
                                set_halign: gtk::Align::Start,
                            },

                            attach[1,0,1,1] = &gtk::Label {
                                #[watch]
                                set_label: &dev.device.name,
                                set_selectable: true,
                                set_halign: gtk::Align::Start,
                                set_hexpand: true
                            },

                            attach[0,1,1,1] = &gtk::Label {
                                set_label: "Device phys:",
                                set_halign: gtk::Align::Start,
                            },

                            attach[1,1,1,1] = &gtk::Label {
                                #[watch]
                                set_label: dev.device.phys.as_ref().map_or("(Missing)", |v| v),
                                #[watch]
                                set_selectable: dev.device.phys.is_some(),
                                set_halign: gtk::Align::Start,
                                set_hexpand: true,
                            },

                            attach[0,2,1,1] = &gtk::Label {
                                set_label: "Device path:",
                                set_halign: gtk::Align::Start,
                            },

                            attach[1,2,1,1] = &gtk::Label {
                                #[watch]
                                set_label: &format!("{}", dev.device.path.display()),
                                #[watch]
                                set_selectable: dev.device.phys.is_some(),
                                set_halign: gtk::Align::Start,
                                set_hexpand: true,
                            },
                        }
                    },
                    None => {
                        gtk::Label {
                            set_label: "No device selected"
                        }
                    }
                }
            },

            gtk::ScrolledWindow {
                set_vexpand: true,
                gtk::TextView {
                    set_editable: false,
                    set_vscroll_policy: gtk::ScrollablePolicy::Minimum,
                    set_buffer: Some(&model.text_buf)
                }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = Self {
            device: None,
            text_buf: gtk::TextBuffer::default(),
            is_paused: true,
        };

        if let Some(dev) = init {
            model.set_device(dev, sender.clone());
        }

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match message {
            EventLoggerMsg::Pause => self.is_paused = true,
            EventLoggerMsg::Resume => self.is_paused = false,
            EventLoggerMsg::Clear => {
                self.is_paused = true;
                self.text_buf.set_text("")
            }
            EventLoggerMsg::SetDevice(dev) => self.set_device(dev, sender),
            EventLoggerMsg::ClearDevice => self.clear_device(),
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            EventCommandMsg::NewEvent(key, val) => {
                if !self.is_paused && self.device.is_some() {
                    let new_text = format!("{} {val}\n", EventCode::EV_KEY(key));
                    let mut end_iter = self.text_buf.end_iter();
                    self.text_buf.insert(&mut end_iter, &new_text);
                }
            }
        }
    }
}

impl EventLogger {
    fn set_device(&mut self, dev: DeviceInfo, sender: ComponentSender<Self>) {
        self.is_paused = true;
        self.text_buf.set_text("");
        let (bg_sender, bg_recv) = mpsc::channel();
        self.device = Some(DeviceLoggerState {
            device: dev.clone(),
            bg_task_sender: bg_sender,
        });
        sender.spawn_command(move |cmd_sender| {
            let dev_f = std::fs::File::open(&dev.path).unwrap();
            let input_dev = evdev_rs::Device::new_from_file(dev_f).unwrap();

            loop {
                match bg_recv.try_recv() {
                    Ok(BgTaskMsg::Stop) => break,
                    Err(mpsc::TryRecvError::Disconnected) => break,
                    _ => {}
                }
                let (status, event) = input_dev
                    .next_event(evdev_rs::ReadFlag::NORMAL | evdev_rs::ReadFlag::BLOCKING)
                    .unwrap();
                match status {
                    evdev_rs::ReadStatus::Success => {
                        if let EventCode::EV_KEY(key) = event.event_code {
                            cmd_sender
                                .send(EventCommandMsg::NewEvent(key, event.value))
                                .unwrap();
                        }
                    }
                    evdev_rs::ReadStatus::Sync => break,
                }
            }
        });
    }

    fn clear_device(&mut self) {
        self.is_paused = true;
        self.text_buf.set_text("");
        if let Some(dev_state) = self.device.take() {
            dev_state.bg_task_sender.send(BgTaskMsg::Stop).unwrap();
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

#[allow(deprecated)]
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
