use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Window, WindowBounds,
    WindowOptions,
};
use std::time::{Duration, Instant};
use tempo_speed_control::{
    connect_speed_adapter, disconnect_speed_adapter, get_foreground_process, list_processes,
    set_speed_for_process, start_hotkey_poller, take_hotkey_event, RunningProcess,
};

const SPEEDS: [f64; 7] = [0.25, 0.5, 0.75, 1.0, 2.0, 5.0, 10.0];
const BG: u32 = 0x0d1117;
const PANEL: u32 = 0x151b23;
const LINE: u32 = 0x29333e;
const TEXT: u32 = 0xedf4f5;
const MUTED: u32 = 0x8d9aa7;
const MINT: u32 = 0x87efc2;

struct TempoApp {
    processes: Vec<RunningProcess>,
    selected: Option<RunningProcess>,
    speed_index: usize,
    connected_pid: Option<u32>,
    picker_open: bool,
    status: String,
}

impl TempoApp {
    fn new() -> Self {
        Self {
            processes: list_processes().unwrap_or_default(),
            selected: None,
            speed_index: 3,
            connected_pid: None,
            picker_open: false,
            status:
                "Offline single-player only. Enabling loads a timing hook into the selected game."
                    .into(),
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        match list_processes() {
            Ok(processes) => {
                self.processes = processes;
                self.status = format!("{} running applications found", self.processes.len());
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    fn select(&mut self, process: RunningProcess, cx: &mut Context<Self>) {
        if self.connected_pid.is_some_and(|pid| pid != process.pid) {
            if let Some(pid) = self.connected_pid.take() {
                let _ = disconnect_speed_adapter(pid);
            }
        }
        self.selected = Some(process);
        self.picker_open = false;
        self.status = "Game selected".into();
        cx.notify();
    }

    fn set_speed(&mut self, index: usize, cx: &mut Context<Self>) {
        self.speed_index = index.min(SPEEDS.len() - 1);
        if let Some(pid) = self.connected_pid {
            if let Err(error) = set_speed_for_process(pid, SPEEDS[self.speed_index], true) {
                self.connected_pid = None;
                self.status = error;
            }
        } else {
            self.status = format!(
                "Selected {}×. Connect the adapter to apply it.",
                SPEEDS[self.speed_index]
            );
        }
        cx.notify();
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        if let Some(pid) = self.connected_pid.take() {
            match disconnect_speed_adapter(pid) {
                Ok(()) => self.status = "Game clock returned to normal speed".into(),
                Err(error) => self.status = error,
            }
        } else if let Some(process) = &self.selected {
            match connect_speed_adapter(process.pid, SPEEDS[self.speed_index]) {
                Ok(message) => {
                    self.connected_pid = Some(process.pid);
                    self.status = message;
                }
                Err(error) => self.status = error,
            }
        } else {
            self.status = "Choose a running game first.".into();
        }
        cx.notify();
    }

    fn handle_hotkey(&mut self, event: u8, cx: &mut Context<Self>) {
        match event {
            1 => match get_foreground_process() {
                Ok(process) => {
                    self.select(process, cx);
                    self.toggle(cx);
                }
                Err(error) => self.status = error,
            },
            2 => self.set_speed((self.speed_index + 1).min(SPEEDS.len() - 1), cx),
            3 => self.set_speed(self.speed_index.saturating_sub(1), cx),
            4 => self.toggle(cx),
            5 => self.set_speed(3, cx),
            _ => return,
        }
        cx.notify();
    }

    fn heartbeat(&mut self, cx: &mut Context<Self>) {
        if let Some(pid) = self.connected_pid {
            if let Err(error) = set_speed_for_process(pid, SPEEDS[self.speed_index], true) {
                self.connected_pid = None;
                self.status = format!("Adapter disconnected: {error}");
                cx.notify();
            }
        }
    }
}

impl Render for TempoApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_name = self
            .selected
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "No game selected".into());
        let selected_detail = self
            .selected
            .as_ref()
            .map(|p| {
                format!(
                    "{} · PID {}",
                    p.executable.as_deref().unwrap_or(&p.name),
                    p.pid
                )
            })
            .unwrap_or_else(|| "Choose a running game process".into());
        let speed = SPEEDS[self.speed_index];
        let connected = self.connected_pid.is_some();

        div()
            .size_full()
            .bg(rgb(BG))
            .p(px(22.0))
            .text_color(rgb(TEXT))
            .font_family("Segoe UI")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(11.0))
                    .child(
                        div()
                            .size(px(28.0))
                            .rounded(px(8.0))
                            .bg(rgb(MINT))
                            .text_color(rgb(BG))
                            .font_weight(gpui::FontWeight::BOLD)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("T"),
                    )
                    .child(
                        div().flex_1().child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("TEMPO"),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(MUTED))
                            .child("GPUI · WINDOWS"),
                    ),
            )
            .child(div().h(px(1.0)).my(px(19.0)).bg(rgb(LINE)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(42.0))
                            .rounded(px(12.0))
                            .bg(rgb(0x26353e))
                            .text_color(rgb(MINT))
                            .font_weight(gpui::FontWeight::BOLD)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                selected_name
                                    .chars()
                                    .take(2)
                                    .collect::<String>()
                                    .to_uppercase(),
                            ),
                    )
                    .child(
                        div().flex_1().min_w(px(0.0)).child(
                            div()
                                .text_size(px(13.0))
                                .child(selected_name.clone())
                                .child(
                                    div()
                                        .mt(px(4.0))
                                        .text_size(px(10.0))
                                        .text_color(rgb(MUTED))
                                        .child(selected_detail),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .id("choose-game")
                            .cursor_pointer()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(rgb(LINE))
                            .bg(rgb(PANEL))
                            .px(px(11.0))
                            .py(px(8.0))
                            .text_size(px(10.0))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh(cx);
                                this.picker_open = !this.picker_open;
                                cx.notify();
                            }))
                            .child(if self.picker_open {
                                "Close"
                            } else {
                                "Choose game"
                            }),
                    ),
            )
            .when(self.picker_open, |el| {
                el.child(
                    div()
                        .id("process-list")
                        .mt(px(10.0))
                        .max_h(px(190.0))
                        .overflow_y_scroll()
                        .rounded(px(9.0))
                        .border_1()
                        .border_color(rgb(LINE))
                        .bg(rgb(PANEL))
                        .children(self.processes.iter().cloned().map(|process| {
                            let label = format!("{}   ·   PID {}", process.name, process.pid);
                            div()
                                .id(("process", process.pid))
                                .cursor_pointer()
                                .px(px(10.0))
                                .py(px(8.0))
                                .text_size(px(10.0))
                                .text_color(rgb(TEXT))
                                .hover(|style| style.bg(rgb(0x202a34)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select(process.clone(), cx)
                                }))
                                .child(label)
                        })),
                )
            })
            .child(div().h(px(1.0)).my(px(18.0)).bg(rgb(LINE)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Game speed"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(if connected { MINT } else { 0xe5bd73 }))
                            .child(if connected {
                                "● Adapter connected"
                            } else {
                                "● Not connected"
                            }),
                    ),
            )
            .child(
                div()
                    .mt(px(20.0))
                    .mb(px(17.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(58.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(MINT))
                            .child(format!("{speed}×")),
                    )
                    .child(
                        div()
                            .mt(px(5.0))
                            .text_size(px(10.0))
                            .text_color(rgb(MUTED))
                            .child("SELECTED MULTIPLIER"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .children(SPEEDS.iter().enumerate().map(|(index, value)| {
                        let label = format!("{value}");
                        let active = index == self.speed_index;
                        div()
                            .id(("speed", index))
                            .flex_1()
                            .cursor_pointer()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(rgb(if active { 0x4eaa80 } else { LINE }))
                            .bg(rgb(if active { 0x19362b } else { PANEL }))
                            .py(px(10.0))
                            .text_size(px(10.0))
                            .text_color(rgb(if active { MINT } else { MUTED }))
                            .flex()
                            .justify_center()
                            .on_click(cx.listener(move |this, _, _, cx| this.set_speed(index, cx)))
                            .child(label)
                    })),
            )
            .child(
                div()
                    .mt(px(20.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .id("reset-speed")
                            .cursor_pointer()
                            .text_size(px(10.0))
                            .text_color(rgb(MUTED))
                            .on_click(cx.listener(|this, _, _, cx| this.set_speed(3, cx)))
                            .child("↺  Reset to normal"),
                    )
                    .child(
                        div()
                            .id("toggle-speed")
                            .cursor_pointer()
                            .rounded(px(9.0))
                            .bg(rgb(if connected { 0x244636 } else { 0x303944 }))
                            .px(px(12.0))
                            .py(px(8.0))
                            .text_size(px(10.0))
                            .text_color(rgb(TEXT))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle(cx)))
                            .child(if connected {
                                "Speed control on"
                            } else {
                                "Enable speed control"
                            }),
                    ),
            )
            .child(
                div()
                    .mt(px(17.0))
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(rgb(0x3f3829))
                    .bg(rgb(0x1d1b17))
                    .p(px(11.0))
                    .text_size(px(9.0))
                    .text_color(rgb(0xe7c984))
                    .child("ⓘ  Offline single-player only. Compatibility varies by game."),
            )
            .child(
                div()
                    .mt(px(10.0))
                    .text_size(px(9.0))
                    .text_color(rgb(MUTED))
                    .child(self.status.clone()),
            )
    }
}

fn main() {
    start_hotkey_poller();
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.0), px(520.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Game Speed Control");
                cx.new(|cx| {
                    let task = cx.spawn(
                        |this: gpui::WeakEntity<TempoApp>, cx: &mut gpui::AsyncApp| {
                            let mut app = cx.clone();
                            async move {
                                let mut last_heartbeat = Instant::now();
                                loop {
                                    app.background_executor()
                                        .timer(Duration::from_millis(24))
                                        .await;
                                    let event = take_hotkey_event();
                                    if event != 0 {
                                        let _ = this.update(&mut app, |this, cx| {
                                            this.handle_hotkey(event, cx)
                                        });
                                    }
                                    if last_heartbeat.elapsed() >= Duration::from_millis(500) {
                                        last_heartbeat = Instant::now();
                                        let _ =
                                            this.update(&mut app, |this, cx| this.heartbeat(cx));
                                    }
                                }
                            }
                        },
                    );
                    task.detach();
                    TempoApp::new()
                })
            },
        )
        .expect("failed to open Tempo window");
    });
}
