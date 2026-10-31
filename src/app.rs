// SPDX-License-Identifier: MPL-2.0

use std::time::{Duration, SystemTime};

use cosmic::Element;
use cosmic::app::Task;
use cosmic::cosmic_config;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Alignment, Length, Limits, Subscription};
use cosmic::widget;

use crate::config::{self, Config};
use crate::fl;
use crate::sound;
use crate::state::{self, State};
use crate::timer::{Event, Timer, TimerState, format_remaining};

const ICON_NAME: &str = "alarm-symbolic";

pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    timer: Timer,
    config: Config,
    /// Where the timer is persisted; `None` if the state store could not be opened.
    state_handler: Option<cosmic_config::Config>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Start,
    Pause,
    Resume,
    Reset,
    Tick,
    SoundPlayed,
    SetDuration(Duration),
    SetMinutes(u64),
    SetSeconds(u64),
    ConfigChanged(Config),
}

impl AppModel {
    /// Handles a message and returns the follow-up task. Called by `update`,
    /// which saves the timer afterwards if the message changed it.
    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => return self.toggle_popup(),
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Start => self.timer.start(SystemTime::now()),
            Message::Pause => self.timer.pause(SystemTime::now()),
            Message::Resume => self.timer.resume(SystemTime::now()),
            Message::Reset => self.timer.reset(),
            Message::Tick => {
                if let Some(Event::Expired) = self.timer.tick(SystemTime::now()) {
                    return cosmic::task::future(async {
                        sound::play_expired().await;
                        Message::SoundPlayed
                    });
                }
            }
            Message::SoundPlayed => {}
            Message::SetDuration(duration) => self.timer.set_duration(duration),
            Message::SetMinutes(minutes) => {
                let seconds = self.timer.duration.as_secs() % 60;
                self.timer
                    .set_duration(Duration::from_secs(minutes * 60 + seconds));
            }
            Message::SetSeconds(seconds) => {
                let minutes = self.timer.duration.as_secs() / 60;
                self.timer
                    .set_duration(Duration::from_secs(minutes * 60 + seconds));
            }
            Message::ConfigChanged(config) => self.config = config,
        }
        Task::none()
    }

    fn toggle_popup(&mut self) -> Task<Message> {
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

    /// The buttons shown in the popup for the current timer state.
    fn controls(&self) -> Element<'_, Message> {
        let row = match self.timer.state {
            TimerState::Idle => {
                let can_start = !self.timer.duration.is_zero();
                widget::Row::new().push(
                    widget::button::suggested(fl!("start"))
                        .on_press_maybe(can_start.then_some(Message::Start)),
                )
            }
            TimerState::Running { .. } => widget::Row::new()
                .push(widget::button::standard(fl!("pause")).on_press(Message::Pause))
                .push(widget::button::destructive(fl!("reset")).on_press(Message::Reset)),
            TimerState::Paused { .. } => widget::Row::new()
                .push(widget::button::suggested(fl!("resume")).on_press(Message::Resume))
                .push(widget::button::destructive(fl!("reset")).on_press(Message::Reset)),
            TimerState::Expired => widget::Row::new()
                .push(widget::button::suggested(fl!("done")).on_press(Message::Reset)),
        };
        row.spacing(8).into()
    }

    /// One preset button, e.g. "5 min", that sets the duration when pressed, enabled only while idle.
    fn preset_button(&self, minutes: u64) -> Element<'_, Message> {
        let idle = matches!(self.timer.state, TimerState::Idle);
        let message = idle.then(|| Message::SetDuration(Duration::from_mins(minutes)));

        widget::button::standard(fl!("preset-minutes", minutes = minutes))
            .on_press_maybe(message)
            .into()
    }

    /// The preset row: quick durations from the config, enabled only while idle.
    fn presets(&self) -> Element<'_, Message> {
        let buttons = self
            .config
            .presets
            .iter()
            .map(|&minutes| self.preset_button(minutes));
        widget::Row::with_children(buttons).spacing(8).into()
    }

    /// Minutes and seconds spin buttons for a custom duration.
    fn manual_entry(&self) -> Element<'_, Message> {
        let total = self.timer.duration.as_secs();
        let minutes = total / 60;
        let seconds = total % 60;

        widget::Row::new()
            .push(widget::spin_button(
                fl!("minutes-value", minutes = minutes),
                minutes,
                1,
                0,
                999,
                Message::SetMinutes,
            ))
            .push(widget::spin_button(
                fl!("seconds-value", seconds = seconds),
                seconds,
                1,
                0,
                59,
                Message::SetSeconds,
            ))
            .spacing(8)
            .into()
    }
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
        let (state_handler, state) = state::load(Self::APP_ID);
        let app = AppModel {
            core,
            popup: None,
            timer: state.timer,
            config: config::load(Self::APP_ID),
            state_handler,
        };
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Message> {
        let idle = matches!(self.timer.state, TimerState::Idle);
        let expired = matches!(self.timer.state, TimerState::Expired);

        // An expired timer stands out on the accent colour and a click
        // acknowledges it instead of opening the popup.
        let class = if expired {
            cosmic::theme::Button::Suggested
        } else {
            cosmic::theme::Button::AppletIcon
        };
        let on_press = if expired {
            Message::Reset
        } else {
            Message::TogglePopup
        };

        // Icon only: when idle, or on a vertical panel where text does not fit.
        // Both branches go through `autosize_window` so the panel shrinks the
        // window back when the text disappears.
        if idle || !self.core.applet.is_horizontal() {
            let button = self
                .core
                .applet
                .icon_button(ICON_NAME)
                .on_press(on_press)
                .class(class);
            return self.core.applet.autosize_window(button).into();
        }

        let (icon_size, _) = self.core.applet.suggested_size(true);
        let (major_padding, minor_padding) = self.core.applet.suggested_padding(true);
        let button_height = f32::from(icon_size + 2 * minor_padding);

        let icon = widget::icon::from_name(ICON_NAME)
            .symbolic(true)
            .size(icon_size)
            .icon()
            .width(Length::Fixed(f32::from(icon_size)))
            .height(Length::Fixed(f32::from(icon_size)));
        let remaining = format_remaining(self.timer.remaining(SystemTime::now()));
        let content = widget::Row::new()
            .push(icon)
            .push(self.core.applet.text(remaining))
            .align_y(Alignment::Center)
            .spacing(4);

        // `button_from_element` fixes the width to the icon size, which would
        // clip the text; build the button by hand with a free width instead,
        // centring the content in the panel's height.
        let button = widget::button::custom(
            widget::container(content).center_y(Length::Fixed(button_height)),
        )
        .padding([0, major_padding])
        .on_press(on_press)
        .class(class);

        self.core.applet.autosize_window(button).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let now = SystemTime::now();
        let remaining = format_remaining(self.timer.remaining(now));
        let progress = self.timer.progress(now);

        let content = widget::Column::new()
            .push(widget::text::title1(remaining))
            .push(widget::progress_bar::determinate_linear(progress))
            .push(self.controls())
            .push(widget::divider::horizontal::default())
            .push(self.presets())
            .push(self.manual_entry())
            .align_x(Alignment::Center)
            .spacing(12)
            .padding(12);

        self.core.applet.popup_container(content).into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Save on transitions only: a tick leaves the timer untouched until it
        // expires, so comparing before and after never writes on every tick.
        let before = self.timer.clone();
        let task = self.handle(message);
        if self.timer != before
            && let Some(handler) = &self.state_handler
        {
            state::save(
                handler,
                &State {
                    timer: self.timer.clone(),
                },
            );
        }
        task
    }

    fn subscription(&self) -> Subscription<Message> {
        let tick = if matches!(self.timer.state, TimerState::Running { .. }) {
            cosmic::iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };
        let config = self
            .core
            .watch_config::<Config>(Self::APP_ID)
            .map(|update| Message::ConfigChanged(update.config));
        Subscription::batch([tick, config])
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
