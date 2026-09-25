// SPDX-License-Identifier: MPL-2.0

use cosmic::Element;
use cosmic::app::Task;
use cosmic::iced::Limits;
use cosmic::iced::window::Id;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::widget;

use crate::fl;

pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "io.github.angelinuxx.CosmicExtAppletTimer";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let app = AppModel { core, popup: None };
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Message> {
        self.core
            .applet
            .icon_button("alarm-symbolic")
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let content = widget::container(widget::text::title3(fl!("app-title"))).padding(12);
        self.core.applet.popup_container(content).into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => {
                if let Some(id) = self.popup.take() {
                    destroy_popup(id)
                } else {
                    let id = Id::unique();
                    self.popup = Some(id);
                    let mut settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        id,
                        None,
                        None,
                        None,
                    );
                    settings.positioner.size_limits = Limits::NONE
                        .min_width(200.0)
                        .max_width(400.0)
                        .min_height(80.0)
                        .max_height(400.0);
                    get_popup(settings)
                }
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                Task::none()
            }
        }
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
