use std::{error::Error, sync::mpsc};

use evdev_rs::enums::EventCode;
use gtk::prelude::*;
use relm4::{Sender, prelude::*};

use crate::{deviceinfo::DeviceInfo, evdev_utils::KeyCode};

#[derive(Debug)]
pub struct EventLogger {
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
pub enum EventLoggerMsg {
    Pause,
    Resume,
    Clear,
    SetDevice(DeviceInfo),
    ClearDevice,
}

#[derive(Debug)]
pub enum EventCommandMsg {
    NewEvent(KeyCode, i32),
    ErrorOccured(std::io::Error),
}

#[derive(Debug)]
pub enum EventLoggerOutput {
    ErrorOccured(Box<dyn Error + Send + 'static>, Option<String>),
}

#[relm4::component(pub)]
impl Component for EventLogger {
    type Init = Option<DeviceInfo>;
    type Input = EventLoggerMsg;
    type Output = EventLoggerOutput;
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
                                set_hexpand: true,
                                set_ellipsize: gtk::pango::EllipsizeMode::End
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
                                set_ellipsize: gtk::pango::EllipsizeMode::End
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
                                set_ellipsize: gtk::pango::EllipsizeMode::End
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
        sender: ComponentSender<Self>,
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
            EventCommandMsg::ErrorOccured(e) => sender
                .output(EventLoggerOutput::ErrorOccured(
                    Box::new(e),
                    Some("Event logger error".to_owned()),
                ))
                .unwrap(),
        }
    }
}

impl EventLogger {
    fn event_logger_task(
        cmd_sender: Sender<EventCommandMsg>,
        dev: DeviceInfo,
        bg_recv: mpsc::Receiver<BgTaskMsg>,
    ) -> std::io::Result<()> {
        let dev_f = std::fs::File::open(&dev.path)?;
        let input_dev = evdev_rs::Device::new_from_file(dev_f)?;

        loop {
            match bg_recv.try_recv() {
                Ok(BgTaskMsg::Stop) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
                _ => {}
            }
            let (status, event) =
                input_dev.next_event(evdev_rs::ReadFlag::NORMAL | evdev_rs::ReadFlag::BLOCKING)?;
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
        Ok(())
    }

    fn set_device(&mut self, dev: DeviceInfo, sender: ComponentSender<Self>) {
        self.is_paused = true;
        self.text_buf.set_text("");
        let (bg_sender, bg_recv) = mpsc::channel();
        self.device = Some(DeviceLoggerState {
            device: dev.clone(),
            bg_task_sender: bg_sender,
        });
        sender.spawn_command(move |cmd_sender| {
            let res = Self::event_logger_task(cmd_sender.clone(), dev, bg_recv);
            if let Err(e) = res {
                let _ = cmd_sender.send(EventCommandMsg::ErrorOccured(e));
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
