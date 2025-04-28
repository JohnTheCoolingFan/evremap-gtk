use std::{
    collections::{HashMap, HashSet},
    env::VarError,
    error::Error,
    path::PathBuf,
    str::FromStr,
};

use components::{
    device_browser::{DeviceDisplay, DeviceDisplayMsg, DeviceDisplayOutput},
    dual_role::{DualRoleMapItem, DualRoleMapItemOutput},
    event_logger::{EventLogger, EventLoggerMsg, EventLoggerOutput},
    key_seq::KeySeqInputMsg,
    remap::{RemapItem, RemapItemOutput},
};
use config_file::{ConfigFile, DualRoleConfig, RemapConfig};
use deviceinfo::DeviceInfo;
use gtk::{self, glib, prelude::*};
use log::LevelFilter;
use relm4::{abstractions::Toaster, adw, factory::FactoryVecDequeGuard, prelude::*};
use relm4_components::{
    open_dialog::{OpenDialog, OpenDialogMsg, OpenDialogResponse, OpenDialogSettings},
    save_dialog::{SaveDialog, SaveDialogMsg, SaveDialogResponse, SaveDialogSettings},
};

mod components;
mod evdev_utils;
mod key_combo;

mod config_file;
mod deviceinfo;

// TODO:
//  - Localized key names? Would be a big change, as a support for localizations woudl be needed,
//    but in theory adding localizations of other parts of the application wouldn't be so hard after
//    that. The biggest problem would be collecting enough localizations. For keys, there are
//    probably pre-existing localizations, such as for GNOME's settings interface. Same could be
//    said about the usual buttons such as "Save As" and "Open". The chances for finding good
//    pre-existing localizations for other messages and labels are neraly 0, and I can only provide
//    english and russian myself.

const APP_ID: &str = "ru.jtcf.evremap_gtk";

/// Initialize logging for the `log` crate via glib's logging
fn init_logging() {
    if let Err(VarError::NotPresent) = std::env::var("G_MESSAGES_DEBUG") {
        // SAFETY: first function called in `main`, no other threads are spawned yet, including
        // those possibly spawned by starting the gtk/relm4 application
        unsafe {
            std::env::set_var("G_MESSAGES_DEBUG", "evremap_gtk");
        }
    }

    static GLIB_LOGGER: glib::GlibLogger = glib::GlibLogger::new(
        glib::GlibLoggerFormat::Plain,
        glib::GlibLoggerDomain::CrateTarget,
    );

    let log_level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|lvl| {
            LevelFilter::from_str(&lvl)
                .inspect_err(|e| {
                    println!("Failed to parse the supplied log level: {e}");
                })
                .ok()
        })
        .unwrap_or(LevelFilter::Warn);

    let _ = log::set_logger(&GLIB_LOGGER);
    log::set_max_level(log_level);

    log::debug!("Logging set up finished!")
}

fn main() {
    init_logging();
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

/// Contains the entry buffers for the device name and phys text entries, stored in the [`AppModel`]
/// for easy access when needed for saving
#[derive(Debug, Default)]
struct ConfigFileGtkBuf {
    name: gtk::EntryBuffer,
    phys: gtk::EntryBuffer,
}

impl ConfigFileGtkBuf {
    /// Update the entry buffers from a parsed config file
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

    /// Extract text from entry buffers and add the remap configs to form a config file for later
    /// saving it
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
    /// Update the list of devices in the browser
    UpdateDeviceList(Vec<DeviceInfo>),
    DeviceListRefreshError(Box<dyn Error + Send + 'static>),
}

#[derive(Debug)]
struct AppModel {
    config: ConfigFileGtkBuf,
    remaps: FactoryVecDeque<RemapItem>,
    dual_role_remaps: FactoryVecDeque<DualRoleMapItem>,
    open_dialog: Controller<OpenDialog>,
    save_dialog: Controller<SaveDialog>,
    device_browser: FactoryVecDeque<DeviceDisplay>,
    duplicate_names: HashSet<String>,
    event_logger: Controller<EventLogger>,
    toaster: Toaster,
}

#[derive(Debug)]
enum AppMsg {
    /// Message to trigger a redraw, completely ignored otherwise
    Ignore,
    /// Request to save the config, triggered by the "Save As" button
    SaveRequest,
    /// User has selected a file to save the config to
    SaveResponse(PathBuf),
    /// Request to open a config file from disk
    OpenRequest,
    /// User has selected a config file to parse
    OpenResponse(PathBuf),
    AddRemap,
    DeleteRemap(DynamicIndex),
    AddDualRoleRemap,
    DeleteDualRoleRemap(DynamicIndex),
    /// Copy the device's name and phys to the editor
    SetDevice(DeviceInfo),
    /// Request to update teh list of devices
    RefreshDevices,
    /// Set the device for event logging
    SetLoggerDevice(DeviceInfo),
    /// Display an error in the UI
    ReportError {
        error: Box<dyn Error + Send + 'static>,
        extra_context: Option<String>,
    },
    ShowHiddenDevices,
    HideUselessDevices,
}

impl AppMsg {
    pub fn err_msg<E: Error + Send + 'static, S: Into<String>>(e: E, msg: Option<S>) -> AppMsg {
        AppMsg::ReportError {
            error: Box::new(e),
            extra_context: msg.map(Into::into),
        }
    }
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

            #[local_ref]
            toast_overlay -> adw::ToastOverlay {
                set_vexpand: true,

                #[name(contents_stack)]
                gtk::Stack {
                    add_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_margin_all: 12,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 6,
                            set_hexpand: true,
                            #[name = "device_name_entry"]
                            gtk::Entry {
                            set_hexpand: true,
                                set_placeholder_text: Some("Device name (required)"),
                                set_buffer: &model.config.name,
                                connect_changed => AppMsg::Ignore,
                                #[watch]
                                set_class_active: ("warning", model.should_display_name_warning()),
                            },

                            gtk::Image::from_icon_name("dialog-warning-symbolic") {
                                #[watch]
                                set_visible: model.should_display_name_warning(),
                                set_margin_all: 6,
                                set_tooltip_text: Some("Multiple devices with this name are currently connected\nSpecifying the phys is recommended")
                            }
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

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 6,
                            set_margin_all: 6,

                            #[name(hidden_devs_toggle)]
                            gtk::CheckButton::with_label("Hide devices without supported events") {
                                set_active: true,
                                connect_toggled[sender] => move |cb| {
                                    sender.input(
                                    match cb.is_active() {
                                        true => AppMsg::HideUselessDevices,
                                        false => AppMsg::ShowHiddenDevices,
                                    })
                                }
                            }
                        },

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

        let event_logger =
            EventLogger::builder()
                .launch(None)
                .forward(sender.input_sender(), |out| match out {
                    EventLoggerOutput::ErrorOccured(e, msg) => AppMsg::ReportError {
                        error: e,
                        extra_context: msg,
                    },
                });

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
            duplicate_names: HashSet::new(),
            event_logger,
            toaster: Toaster::default(),
        };

        let remaps_box = model.remaps.widget();
        let dual_role_box = model.dual_role_remaps.widget();
        let device_browser_box = model.device_browser.widget();
        let event_logger_box = model.event_logger.widget();
        let toast_overlay = model.toaster.overlay_widget();
        let widgets = view_output!();

        relm4::ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match message {
            AppMsg::Ignore => {}
            AppMsg::SaveRequest => self.save_dialog.emit(SaveDialogMsg::Save),
            AppMsg::SaveResponse(path) => {
                if let Err(e) = self.to_config_file().save_to(path) {
                    sender.input(AppMsg::err_msg(e, Some("Failed to save config file")))
                }
            }
            AppMsg::OpenRequest => self.open_dialog.emit(OpenDialogMsg::Open),
            AppMsg::OpenResponse(path) => match ConfigFile::read_from(path) {
                Ok(config) => self.load(config),
                Err(e) => sender.input(AppMsg::err_msg(e, Some("Failed to open selected file"))),
            },
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
                sender.spawn_oneshot_command(|| match DeviceInfo::obtain_device_list() {
                    Ok(devices) => CommandMsg::UpdateDeviceList(devices),
                    Err(e) => CommandMsg::DeviceListRefreshError(Box::new(e)),
                });
            }
            AppMsg::SetLoggerDevice(dev) => {
                self.event_logger.emit(EventLoggerMsg::SetDevice(dev));
            }
            AppMsg::ReportError {
                error,
                extra_context,
            } => self.show_error_toast(error, extra_context),
            AppMsg::ShowHiddenDevices => {
                self.device_browser.broadcast(DeviceDisplayMsg::ShowHidden)
            }
            AppMsg::HideUselessDevices => {
                self.device_browser.broadcast(DeviceDisplayMsg::HideUseless)
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CommandMsg::UpdateDeviceList(devices) => {
                // Update the list of device names that have multiple devices associated with them
                let names_counts: HashMap<&str, usize> =
                    devices
                        .iter()
                        .map(|d| &d.name)
                        .fold(HashMap::new(), |mut acc, dname| {
                            *acc.entry(dname).or_insert(0) += 1;
                            acc
                        });
                self.duplicate_names.clear();
                self.duplicate_names.extend(
                    names_counts
                        .into_iter()
                        .filter(|&(_dname, count)| (count > 1))
                        .map(|(dname, _count)| dname.to_owned()),
                );
                // Clear the device browser list and add each device
                let mut device_list = self.device_browser.guard();
                device_list.clear();
                for dev in devices {
                    device_list.push_back(dev);
                }
            }
            CommandMsg::DeviceListRefreshError(e) => sender.input(AppMsg::ReportError {
                error: e,
                extra_context: Some("Failed to refresh the device list".to_owned()),
            }),
        }
    }
}

impl AppModel {
    /// Load config data from a parsed config file
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

    /// Collect the data from buffers and factories to form a config file for saving
    fn to_config_file(&self) -> ConfigFile {
        let remaps = self.remaps_extract();
        let dual_remaps = self.dual_remaps_extract();
        self.config.to_config_file(remaps, dual_remaps)
    }

    fn remaps_extract(&self) -> Vec<RemapConfig> {
        self.remaps
            .iter()
            .map(|remap_item| RemapConfig {
                input: remap_item.input_seq.model().sequence.to_keys(),
                output: remap_item.output_seq.model().sequence.to_keys(),
            })
            .collect()
    }

    fn dual_remaps_extract(&self) -> Vec<DualRoleConfig> {
        self.dual_role_remaps
            .iter()
            .map(|dual_role| DualRoleConfig {
                input: dual_role.key,
                hold: dual_role.hold_seq.model().sequence.to_keys(),
                tap: dual_role.tap_seq.model().sequence.to_keys(),
            })
            .collect()
    }

    fn show_error_toast(&self, error: Box<dyn Error + Send + 'static>, ctx: Option<String>) {
        let error_msg = match ctx {
            Some(ctx) => {
                format!("{ctx}: {error}")
            }
            None => {
                format!("Error occured: {error}")
            }
        };
        let toast = adw::Toast::builder()
            .title(&error_msg)
            .button_label("Dismiss")
            .timeout(10)
            .build();
        toast.connect_button_clicked(move |tst| tst.dismiss());
        self.toaster.add_toast(toast);
    }

    /// Display the warning about the device name if there are multiple devices with this name
    /// connected AND phys is not specified.
    fn should_display_name_warning(&self) -> bool {
        self.duplicate_names
            .contains(self.config.name.text().as_str())
            && self.config.phys.text().is_empty()
    }
}
