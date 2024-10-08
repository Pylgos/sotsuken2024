use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Result};
use futures::FutureExt;
use gilrs::Gilrs;
use tokio::{task::JoinHandle, time::sleep};

use eframe::egui::{self, mutex::Mutex, Color32, Rounding, Sense, Stroke, Vec2};
use vrrop_control_client::{Client, SetTargetVelocity};

struct App {
    server_address: String,
    gamepad_loop: Option<std::thread::JoinHandle<Result<()>>>,
    state: Arc<SharedState>,
}

#[derive(Default, Debug, Clone)]
struct GamepadState {
    left_stick_x: f32,
    left_stick_y: f32,
    right_stick_x: f32,
    right_stick_y: f32,
}

struct SharedState {
    ctx: egui::Context,
    selected_gamepad: Mutex<Option<(gilrs::GamepadId, String)>>,
    gamepads: Mutex<Vec<(gilrs::GamepadId, String)>>,
    gamepad_state: Mutex<GamepadState>,
    client_join_handle: Mutex<Option<JoinHandle<()>>>,
    client_error_message: Mutex<Option<String>>,
    shutdown: AtomicBool,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Result<Self> {
        let state = Arc::new(SharedState {
            ctx: cc.egui_ctx.clone(),
            selected_gamepad: Mutex::new(None),
            gamepads: Mutex::new(Vec::new()),
            gamepad_state: Mutex::new(GamepadState::default()),
            client_join_handle: Mutex::new(None),
            client_error_message: Mutex::new(None),
            shutdown: AtomicBool::new(false),
        });
        let gamepad_loop = std::thread::spawn({
            let state = state.clone();
            move || gamepad_event_loop(state)
        });
        Ok(Self {
            gamepad_loop: Some(gamepad_loop),
            state,
            server_address: "127.0.0.1:23456".into(),
        })
    }
}

#[derive(Debug)]
struct GamepadVisualizer {
    state: GamepadState,
}

impl GamepadVisualizer {
    fn new(state: GamepadState) -> Self {
        Self { state }
    }
}

impl egui::Widget for GamepadVisualizer {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let stick = |ui: &mut egui::Ui, label: &str, x: f32, y: f32| {
            ui.vertical(|ui| {
                ui.label(label);
                ui.label(format!("x: {x:5.2}"));
                ui.label(format!("y: {y:5.2}"));
            });
            let (rect, _) = ui.allocate_at_least(Vec2::new(100.0, 100.0), Sense::hover());
            let circle_radius = rect.size().min_elem() / 50.0;
            let mul = (rect.size().min_elem() / 2.0) - circle_radius;
            let left_stick = Vec2::new(x, -y);
            let p = ui.painter();
            p.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0, Color32::WHITE));
            p.line_segment(
                [rect.left_center(), rect.right_center()],
                Stroke::new(1.0, Color32::WHITE),
            );
            p.line_segment(
                [rect.center_top(), rect.center_bottom()],
                Stroke::new(1.0, Color32::WHITE),
            );
            p.circle_filled(
                rect.center() + left_stick * mul,
                circle_radius,
                Color32::RED,
            );
        };
        ui.horizontal(|ui| {
            stick(
                ui,
                "Left Stick",
                self.state.left_stick_x,
                self.state.left_stick_y,
            );
            stick(
                ui,
                "Right Stick",
                self.state.right_stick_x,
                self.state.right_stick_y,
            );
        })
        .response
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let state = &self.state;
            ui.heading("VRROP Control Client Desktop");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.server_address);
                let connect = ui.button("Connect").clicked();
                let disconnect = ui
                    .add_enabled_ui(state.client_join_handle.lock().is_some(), |ui| {
                        ui.button("Disconnect").clicked()
                    })
                    .inner;
                if connect {
                    state.client_error_message.lock().take();
                    let join_handle = tokio::spawn({
                        let addr = self.server_address.clone();
                        let state1 = state.clone();
                        let state2 = state1.clone();
                        async move {
                            let client = Client::new(addr).await?;
                            loop {
                                let gamepad = state1.gamepad_state.lock().clone();
                                client
                                    .set_target_velocity(SetTargetVelocity {
                                        forward: gamepad.left_stick_y,
                                        turn: -gamepad.right_stick_x,
                                    })
                                    .await?;
                                sleep(Duration::from_secs_f64(0.01)).await;
                            }
                        }
                        .map(move |res: Result<()>| {
                            state2.client_join_handle.lock().take();
                            if let Err(err) = res {
                                state2.client_error_message.lock().replace(err.to_string());
                            }
                            state2.ctx.request_repaint();
                        })
                    });
                    if let Some(prev_join_handle) =
                        state.client_join_handle.lock().replace(join_handle)
                    {
                        prev_join_handle.abort();
                    }
                }
                if disconnect {
                    if let Some(jh) = state.client_join_handle.lock().take() {
                        jh.abort();
                    }
                    state.client_error_message.lock().take();
                }
            });
            {
                let connected = state.client_join_handle.lock().is_some();
                let error = state.client_error_message.lock();
                let (color, text) = if connected {
                    (Color32::GREEN, "Connected")
                } else if let Some(error) = error.as_ref() {
                    (Color32::RED, error.as_str())
                } else {
                    (Color32::WHITE, "Disconnected")
                };
                ui.colored_label(color, text);
            }
            egui::ComboBox::from_label("Controller")
                .selected_text({
                    let selected_gamepad = state.selected_gamepad.lock();
                    selected_gamepad
                        .as_ref()
                        .map(|(_, name)| name.clone())
                        .unwrap_or_else(|| "None".to_string())
                })
                .show_ui(ui, |ui| {
                    let mut selected_gamepad = state.selected_gamepad.lock();
                    let gamepads = state.gamepads.lock();
                    ui.selectable_value(&mut *selected_gamepad, None, "None");
                    for (id, name) in gamepads.iter() {
                        ui.selectable_value(
                            &mut *selected_gamepad,
                            Some((*id, name.clone())),
                            name,
                        );
                    }
                });
            ui.add(GamepadVisualizer::new(state.gamepad_state.lock().clone()));
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.state.shutdown.store(true, Ordering::Relaxed);
        if let Some(jh) = self.gamepad_loop.take() {
            jh.join().unwrap().unwrap()
        };
    }
}

fn gamepad_event_loop(state: Arc<SharedState>) -> Result<()> {
    let mut gilrs = gilrs::Gilrs::new().map_err(|err| anyhow!("{err}"))?;

    let update_gamepads = |gilrs: &Gilrs| {
        let mut gamepads = state.gamepads.lock();
        gamepads.clear();
        for (id, gamepad) in gilrs.gamepads() {
            gamepads.push((id, gamepad.name().to_string()));
        }
        let mut selected_gamepad = state.selected_gamepad.lock();
        if selected_gamepad.is_none() {
            *selected_gamepad = gamepads.first().cloned();
        }
        state.ctx.request_repaint();
    };

    update_gamepads(&gilrs);

    loop {
        if let Some(event) = gilrs.next_event_blocking(Some(Duration::from_millis(100))) {
            use gilrs::Axis::*;
            use gilrs::EventType::*;
            match event.event {
                Connected | Disconnected => {
                    update_gamepads(&gilrs);
                }
                _ => {}
            }
            if Some(event.id) == state.selected_gamepad.lock().as_ref().map(|(id, _)| *id) {
                let mut gamepad_state = state.gamepad_state.lock();
                match event.event {
                    AxisChanged(axis, value, _) => match axis {
                        LeftStickX => gamepad_state.left_stick_x = value,
                        LeftStickY => gamepad_state.left_stick_y = value,
                        RightStickX => gamepad_state.right_stick_x = value,
                        RightStickY => gamepad_state.right_stick_y = value,
                        _ => {}
                    },
                    _ => {}
                }
                state.ctx.request_repaint();
            }
        }
        if state.shutdown.load(Ordering::Relaxed) {
            break;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "VRROP Control Client Desktop",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)?))),
    )
    .map_err(|err| anyhow!("{err}"))
}
