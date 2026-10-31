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
use crate::timer::{Timer, TimerState, format_remaining, panel_timer};

const ICON_NAME: &str = "alarm-symbolic";
const REMOVE_ICON_NAME: &str = "edit-delete-symbolic";
/// Leaves room for the Add button inside the popup's 700 px maximum height.
const MAX_LIST_HEIGHT: f32 = 560.0;

pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    /// The timers and the next free id; persisted whenever it changes.
    state: State,
    config: Config,
    /// Where the settings are written; `None` if the store could not be opened.
    config_handler: Option<cosmic_config::Config>,
    /// Where the state is persisted; `None` if the state store could not be opened.
    state_handler: Option<cosmic_config::Config>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Tick,
    SoundPlayed,
    ConfigChanged(Config),
    SetSoundEnabled(bool),
    AddTimer,
    RemoveTimer(u32),
    /// Panel click on an expired timer: resets every expired timer.
    AcknowledgeExpired,
    /// An action on the timer with the given id.
    Timer(u32, TimerAction),
}

#[derive(Debug, Clone)]
pub enum TimerAction {
    Start,
    Pause,
    Resume,
    Reset,
    SetDuration(Duration),
    SetMinutes(u64),
    SetSeconds(u64),
    SetLabel(String),
}

impl TimerAction {
    fn apply(self, timer: &mut Timer) {
        match self {
            Self::Start => timer.start(SystemTime::now()),
            Self::Pause => timer.pause(SystemTime::now()),
            Self::Resume => timer.resume(SystemTime::now()),
            Self::Reset => timer.reset(),
            Self::SetDuration(duration) => timer.set_duration(duration),
            Self::SetMinutes(minutes) => {
                let seconds = timer.duration.as_secs() % 60;
                timer.set_duration(Duration::from_secs(minutes * 60 + seconds));
            }
            Self::SetSeconds(seconds) => {
                let minutes = timer.duration.as_secs() / 60;
                timer.set_duration(Duration::from_secs(minutes * 60 + seconds));
            }
            Self::SetLabel(label) => timer.label = label,
        }
    }
}

impl AppModel {
    /// Handles a message and returns the follow-up task. Called by `update`,
    /// which saves the state afterwards if the message changed it.
    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => return self.toggle_popup(),
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Tick => {
                let now = SystemTime::now();
                // Tick every timer (no short-circuit), then play the sound
                // once even if several expired together.
                let mut expired = false;
                for timer in &mut self.state.timers {
                    expired |= timer.tick(now).is_some();
                }
                if expired && self.config.sound_enabled {
                    return cosmic::task::future(async {
                        sound::play_expired().await;
                        Message::SoundPlayed
                    });
                }
            }
            Message::SoundPlayed => {}
            Message::ConfigChanged(config) => self.config = config,
            Message::SetSoundEnabled(enabled) => {
                if let Some(handler) = &self.config_handler
                    && let Err(err) = self.config.set_sound_enabled(handler, enabled)
                {
                    eprintln!("failed to save sound setting: {err}");
                }
            }
            Message::AddTimer => {
                let minutes = self.config.presets.first().copied().unwrap_or(5);
                let timer = Timer::new(self.state.next_id, Duration::from_mins(minutes));
                self.state.timers.push(timer);
                self.state.next_id += 1;
            }
            Message::RemoveTimer(id) => {
                // The popup never offers to remove the last timer.
                if self.state.timers.len() > 1 {
                    self.state.timers.retain(|timer| timer.id != id);
                }
            }
            Message::AcknowledgeExpired => {
                for timer in &mut self.state.timers {
                    if matches!(timer.state, TimerState::Expired) {
                        timer.reset();
                    }
                }
            }
            Message::Timer(id, action) => {
                if let Some(timer) = self.state.timers.iter_mut().find(|timer| timer.id == id) {
                    action.apply(timer);
                }
            }
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
                .max_height(700.0);
            get_popup(settings)
        }
    }

    /// One card in the popup: label, remaining time, progress, controls and,
    /// while idle, the duration pickers.
    fn timer_card<'a>(&'a self, timer: &'a Timer, now: SystemTime) -> Element<'a, Message> {
        let id = timer.id;
        let label = widget::text_input(fl!("label-placeholder"), &timer.label)
            .on_input(move |label| Message::Timer(id, TimerAction::SetLabel(label)))
            .width(Length::Fill);
        let removable = self.state.timers.len() > 1;
        let remove = removable.then(|| {
            widget::button::icon(widget::icon::from_name(REMOVE_ICON_NAME))
                .on_press(Message::RemoveTimer(id))
        });
        let header = widget::Row::new()
            .push(label)
            .push_maybe(remove)
            .align_y(Alignment::Center)
            .spacing(8);

        let idle = matches!(timer.state, TimerState::Idle);
        let content = widget::Column::new()
            .push(header)
            .push(widget::text::title3(format_remaining(timer.remaining(now))))
            .push(widget::progress_bar::determinate_linear(
                timer.progress(now),
            ))
            .push(Self::controls(timer))
            .push_maybe(idle.then(|| self.presets(timer)))
            .push_maybe(idle.then(|| Self::manual_entry(timer)))
            .align_x(Alignment::Center)
            .spacing(8);

        widget::container(content)
            .padding(12)
            .class(cosmic::theme::Container::Card)
            .into()
    }

    /// The buttons shown in a card for the timer's state.
    fn controls(timer: &Timer) -> Element<'_, Message> {
        let id = timer.id;
        let row =
            match timer.state {
                TimerState::Idle => {
                    let can_start = !timer.duration.is_zero();
                    widget::Row::new().push(widget::button::suggested(fl!("start")).on_press_maybe(
                        can_start.then_some(Message::Timer(id, TimerAction::Start)),
                    ))
                }
                TimerState::Running { .. } => widget::Row::new()
                    .push(
                        widget::button::standard(fl!("pause"))
                            .on_press(Message::Timer(id, TimerAction::Pause)),
                    )
                    .push(
                        widget::button::destructive(fl!("reset"))
                            .on_press(Message::Timer(id, TimerAction::Reset)),
                    ),
                TimerState::Paused { .. } => widget::Row::new()
                    .push(
                        widget::button::suggested(fl!("resume"))
                            .on_press(Message::Timer(id, TimerAction::Resume)),
                    )
                    .push(
                        widget::button::destructive(fl!("reset"))
                            .on_press(Message::Timer(id, TimerAction::Reset)),
                    ),
                TimerState::Expired => widget::Row::new().push(
                    widget::button::suggested(fl!("done"))
                        .on_press(Message::Timer(id, TimerAction::Reset)),
                ),
            };
        row.spacing(8).into()
    }

    /// The preset row: quick durations from the config.
    fn presets(&self, timer: &Timer) -> Element<'_, Message> {
        let id = timer.id;
        let buttons = self.config.presets.iter().map(|&minutes| {
            widget::button::standard(fl!("preset-minutes", minutes = minutes))
                .on_press(Message::Timer(
                    id,
                    TimerAction::SetDuration(Duration::from_mins(minutes)),
                ))
                .into()
        });
        widget::Row::with_children(buttons).spacing(8).into()
    }

    /// Minutes and seconds spin buttons for a custom duration.
    fn manual_entry(timer: &Timer) -> Element<'_, Message> {
        let id = timer.id;
        let total = timer.duration.as_secs();
        let minutes = total / 60;
        let seconds = total % 60;

        widget::Row::new()
            .push(widget::spin_button(
                fl!("minutes-value", minutes = minutes),
                minutes,
                1,
                0,
                999,
                move |minutes| Message::Timer(id, TimerAction::SetMinutes(minutes)),
            ))
            .push(widget::spin_button(
                fl!("seconds-value", seconds = seconds),
                seconds,
                1,
                0,
                59,
                move |seconds| Message::Timer(id, TimerAction::SetSeconds(seconds)),
            ))
            .spacing(8)
            .into()
    }
}

/// Panel text for a paused timer: the applet's usual foreground colour from
/// the active theme, at half opacity.
fn dimmed_text(theme: &cosmic::Theme) -> cosmic::iced::widget::text::Style {
    let cosmic = theme.cosmic();
    let mut color: cosmic::iced::Color = cosmic.background(theme.transparent).on.into();
    color.a *= 0.5;
    cosmic::iced::widget::text::Style {
        color: Some(color),
        selected_fill: cosmic.accent.base.into(),
        selected_text_color: None,
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
        let (config_handler, config) = config::load(Self::APP_ID);
        let app = AppModel {
            core,
            popup: None,
            state,
            config,
            config_handler,
            state_handler,
        };
        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Message> {
        let now = SystemTime::now();
        let shown = panel_timer(&self.state.timers, now);
        let expired = shown.is_some_and(|timer| matches!(timer.state, TimerState::Expired));

        // An expired timer stands out on the accent colour and a click
        // acknowledges it instead of opening the popup.
        let class = if expired {
            cosmic::theme::Button::Suggested
        } else {
            cosmic::theme::Button::AppletIcon
        };
        let on_press = if expired {
            Message::AcknowledgeExpired
        } else {
            Message::TogglePopup
        };

        // Icon only when every timer is idle. Both branches go through
        // `autosize_window` so the panel shrinks the window back when the text
        // disappears.
        let Some(timer) = shown else {
            let button = self
                .core
                .applet
                .icon_button(ICON_NAME)
                .on_press(on_press)
                .class(class);
            return self.core.applet.autosize_window(button).into();
        };

        let horizontal = self.core.applet.is_horizontal();
        let (icon_size, _) = self.core.applet.suggested_size(true);
        let (major_padding, minor_padding) = self.core.applet.suggested_padding(true);
        // The panel's thickness: height of a horizontal panel, width of a vertical one.
        let thickness = f32::from(icon_size + 2 * minor_padding);

        let icon = widget::icon::from_name(ICON_NAME)
            .symbolic(true)
            .size(icon_size)
            .icon()
            .width(Length::Fixed(f32::from(icon_size)))
            .height(Length::Fixed(f32::from(icon_size)));

        // One line on a horizontal panel; on a vertical one each part of the
        // time ("04", "50") gets its own line to fit the panel's width.
        let remaining = format_remaining(timer.remaining(now));
        let mut lines: Vec<String> = if horizontal {
            vec![remaining]
        } else {
            remaining.split(':').map(str::to_owned).collect()
        };
        // "+N" for the other timers that are not idle; the shown one is active.
        let others = self.state.timers.iter().filter(|t| t.is_active()).count() - 1;
        if others > 0 {
            lines.push(fl!("more-timers", count = others));
        }
        // A paused timer is shown dimmed. It only wins the panel when no timer
        // is running or expired, so the badge is dimmed along with it.
        let paused = matches!(timer.state, TimerState::Paused { .. });
        let texts = lines.into_iter().map(|line| {
            let text = self.core.applet.text(line);
            let text = if paused {
                text.class(cosmic::theme::Text::Custom(dimmed_text))
            } else {
                text
            };
            Element::from(text)
        });
        let children = std::iter::once(Element::from(icon)).chain(texts);

        // `button_from_element` fixes the width to the icon size, which would
        // clip the text; build the button by hand, free along the panel and
        // centred across it.
        let button = if horizontal {
            let content = widget::Row::with_children(children)
                .align_y(Alignment::Center)
                .spacing(4);
            widget::button::custom(widget::container(content).center_y(Length::Fixed(thickness)))
                .padding([0, major_padding])
        } else {
            let content = widget::Column::with_children(children)
                .align_x(Alignment::Center)
                .spacing(2);
            widget::button::custom(widget::container(content).center_x(Length::Fixed(thickness)))
                .padding([major_padding, 0])
        }
        .on_press(on_press)
        .class(class);

        self.core.applet.autosize_window(button).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let now = SystemTime::now();
        let cards = self
            .state
            .timers
            .iter()
            .map(|timer| self.timer_card(timer, now));

        // A column hands its children the remaining height in order, so an
        // uncapped list would take it all and push the Add button out of the
        // popup. The right padding keeps the scrollbar off the cards.
        let list = widget::scrollable(
            widget::Column::with_children(cards)
                .spacing(12)
                .padding([0, 12, 0, 0]),
        );

        let footer = widget::Row::new()
            .push(widget::button::standard(fl!("add-timer")).on_press(Message::AddTimer))
            .push(widget::space::horizontal())
            .push(
                widget::toggler(self.config.sound_enabled)
                    .label(fl!("sound"))
                    .on_toggle(Message::SetSoundEnabled),
            )
            .align_y(Alignment::Center)
            .spacing(8);

        let content = widget::Column::new()
            .push(widget::container(list).max_height(MAX_LIST_HEIGHT))
            .push(footer)
            .align_x(Alignment::Center)
            .spacing(12)
            .padding(12);

        self.core.applet.popup_container(content).into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Save on transitions only: a tick leaves the timers untouched until
        // one expires, so comparing before and after never writes on every tick.
        let before = self.state.clone();
        let task = self.handle(message);
        if self.state != before
            && let Some(handler) = &self.state_handler
        {
            state::save(handler, &self.state);
        }
        task
    }

    fn subscription(&self) -> Subscription<Message> {
        let running = self
            .state
            .timers
            .iter()
            .any(|timer| matches!(timer.state, TimerState::Running { .. }));
        let tick = if running {
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
