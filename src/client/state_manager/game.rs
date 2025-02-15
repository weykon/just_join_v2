use std::{marker::PhantomData, time::Duration};

use bevy::{
    app::AppExit,
    input::mouse::MouseWheel,
    prelude::{
        in_state, AmbientLight, Commands, DespawnRecursiveExt, Entity, EventReader, EventWriter,
        Input, IntoSystemConfigs, KeyCode, Local, NextState, OnEnter, OnExit, Plugin, Query, Res,
        ResMut, Resource, State, States, Update, Vec2, With,
    },
    window::{CursorGrabMode, PrimaryWindow, Window, WindowCloseRequested},
};
use bevy_easy_localize::Localize;
use bevy_egui::{
    egui::{self, epaint::Shadow, Color32},
    EguiContext, EguiContexts, EguiSet, EguiUserTextures,
};
use bevy_renet::renet::{transport::NetcodeTransportError, RenetClient};
use renet_visualizer::{RenetClientVisualizer, RenetVisualizerStyle};

use crate::{
    client::{
        client_sync_players, client_sync_players_state,
        console_commands::ConsoleCommandPlugins,
        filled_object::{setdown_filled_object, ClientFilledObjectnPlugin},
        mesh_display::{mesh_chunk_map_setdown, ClientMeshPlugin},
        player::{
            controller::{CharacterController, CharacterControllerPlugin, ControllerFlag},
            mouse_control::MouseControlPlugin,
            throw_system::deal_with_throw,
            ClientLobby,
        },
        ray_cast::MeshRayCastPlugin,
        sp_mesh_display::SpMeshManagerPlugin,
        tool_bar_manager::ToolBarSyncPlugin,
        ui::{
            staff_rules::staff_rules_ui,
            tool_bar::{tool_bar, ToolBar},
            UiPicResourceManager,
        },
    },
    common::ClientClipSpheresPlugin,
    sky::ClientSkyPlugins,
};

use super::{new_renet_client, notification::Notification, ConnectionAddr, GameState};

#[derive(Default, Resource)]
pub struct TextEditDemo {
    pub input: String,
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum PlayState {
    Main,
    StaffRules,
    //todo 状态栏
    State,
    #[default]
    Disabled,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_state::<PlayState>();
        app.add_systems(OnEnter(GameState::Game), setup);

        // app.insert_resource();
        app.insert_resource(TextEditDemo::default());
        app.insert_resource(RenetClientVisualizer::<200>::new(
            RenetVisualizerStyle::default(),
        ));
        app.add_systems(
            Update,
            (toggle_play_staff_rules, update_visulizer_system).run_if(in_state(GameState::Game)),
        );
        app.add_systems(
            Update,
            staff_rules_ui.run_if(in_state(PlayState::StaffRules)),
        );
        app.add_systems(
            Update,
            (
                egui_center_cursor_system,
                mian_ui,
                controller_tool_bar,
                chat_window,
            )
                .run_if(in_state(PlayState::Main))
                .after(EguiSet::InitContexts),
        );
        // 这里是系统
        app.add_plugins((
            CharacterControllerPlugin,
            ClientClipSpheresPlugin::<CharacterController> { data: PhantomData },
            ClientMeshPlugin,
            ClientSkyPlugins,
            MeshRayCastPlugin,
            ConsoleCommandPlugins,
            MouseControlPlugin,
            ClientFilledObjectnPlugin,
            ToolBarSyncPlugin,
            SpMeshManagerPlugin,
        ));

        app.add_systems(
            Update,
            (
                client_sync_players,
                client_sync_players_state,
                panic_on_error_system,
                deal_with_throw,
            )
                .chain()
                .run_if(bevy_renet::transport::client_connected())
                .run_if(in_state(GameState::Game)),
        );
        app.add_systems(
            Update,
            disconnect_on_close
                .run_if(in_state(GameState::Game))
                .run_if(bevy_renet::transport::client_connected()),
        );
        app.add_systems(
            Update,
            client_do_disconnected
                .run_if(in_state(GameState::Game))
                .run_if(bevy_renet::transport::client_just_diconnected()),
        );
        app.add_systems(
            OnExit(GameState::Game),
            (setdown, mesh_chunk_map_setdown, setdown_filled_object),
        );
    }
}

fn setdown(mut commands: Commands, mut client_lobby: ResMut<ClientLobby>) {
    for (_, info) in client_lobby.players.clone() {
        commands.entity(info.client_entity).despawn_recursive();
    }
    // 清空数据
    *client_lobby.as_mut() = ClientLobby::default();
}

fn setup(
    mut commands: Commands,
    connection_addr: Res<ConnectionAddr>,
    mut play_state: ResMut<NextState<PlayState>>,
    mut flags: ResMut<ControllerFlag>,
) {
    let (client, transport) = new_renet_client(connection_addr.clone());
    commands.insert_resource(client);
    commands.insert_resource(transport);
    commands.insert_resource(AmbientLight {
        brightness: 1.06,
        ..Default::default()
    });
    commands.insert_resource(ClientLobby::default());
    play_state.set(PlayState::Main);
    // 重新进入游戏后可以控制
    flags.flag = true;
}

// 切换合成公式
fn toggle_play_staff_rules(
    state: Res<State<PlayState>>,
    mut play_state: ResMut<NextState<PlayState>>,
    keyboard_input: Res<Input<KeyCode>>,
    mut primary_window: Query<&mut Window, With<PrimaryWindow>>,
    mut flags: ResMut<ControllerFlag>,
) {
    if let Ok(mut window) = primary_window.get_single_mut() {
        if keyboard_input.just_pressed(KeyCode::E) {
            match state.get() {
                PlayState::StaffRules => {
                    flags.flag = true;
                    play_state.set(PlayState::Main);
                    match window.cursor.grab_mode {
                        CursorGrabMode::None => {
                            window.cursor.grab_mode = CursorGrabMode::Confined;
                            window.cursor.visible = false;
                        }
                        _ => {}
                    }
                }
                _ => {
                    flags.flag = false;
                    play_state.set(PlayState::StaffRules);
                    match window.cursor.grab_mode {
                        CursorGrabMode::None => {}
                        _ => {
                            window.cursor.grab_mode = CursorGrabMode::None;
                            window.cursor.visible = true;
                        }
                    }
                }
            }
        }
    }
}

fn update_visulizer_system(
    mut egui_contexts: EguiContexts,
    mut visualizer: ResMut<RenetClientVisualizer<200>>,
    client: Res<RenetClient>,
    mut show_visualizer: Local<bool>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    visualizer.add_network_info(client.network_info());
    if keyboard_input.just_pressed(KeyCode::F1) {
        *show_visualizer = !*show_visualizer;
    }
    if *show_visualizer {
        visualizer.show_window(egui_contexts.ctx_mut());
    }
}

// If any error is found we just panic
fn panic_on_error_system(mut renet_error: EventReader<NetcodeTransportError>) {
    for e in renet_error.iter() {
        panic!("{}", e);
    }
}

fn client_do_disconnected(
    localize: Res<Localize>,
    client: Res<RenetClient>,
    mut play_state: ResMut<NextState<PlayState>>,
    mut game_state: ResMut<NextState<GameState>>,
    // mut menu_state: ResMut<NextState<MenuState>>,
    mut notification: ResMut<Notification>,
) {
    let mut message = "连接异常";
    if let Some(bevy_renet::renet::DisconnectReason::DisconnectedByServer) =
        client.disconnect_reason()
    {
        message = "用户名已经存在";
    }
    notification
        .toasts
        .error(localize.get(message))
        .set_duration(Some(Duration::from_secs(5)));
    play_state.set(PlayState::Disabled);
    game_state.set(GameState::Menu);
}

// 中心十字

// 添加中心十字
pub fn egui_center_cursor_system(
    mut contexts: EguiContexts,
    window_qurey: Query<&mut Window, With<PrimaryWindow>>,
) {
    let ctx = contexts.ctx_mut();

    let Ok(window) = window_qurey.get_single() else{return;};
    let size = Vec2::new(window.width(), window.height());
    // 透明的屏幕！

    egui::CentralPanel::default()
        .frame(frame_transparent())
        .show(ctx, |ui| {
            // 计算十字准星的位置和大小
            let crosshair_size = 20.0;
            let crosshair_pos = egui::Pos2::new(
                size.x / 2.0 - crosshair_size / 2.0,
                size.y / 2.0 - crosshair_size / 2.0,
            );
            // 外边框
            let crosshair_rect =
                egui::Rect::from_min_size(crosshair_pos, egui::Vec2::splat(crosshair_size));

            // 绘制十字准星的竖线
            let line_width = 2.0;
            let line_rect = egui::Rect::from_min_max(
                egui::Pos2::new(
                    crosshair_rect.center().x - line_width / 2.0,
                    crosshair_rect.min.y,
                ),
                egui::Pos2::new(
                    crosshair_rect.center().x + line_width / 2.0,
                    crosshair_rect.max.y,
                ),
            );
            ui.painter()
                .rect_filled(line_rect, 1.0, egui::Color32::WHITE);

            // 绘制十字准星的横线
            let line_rect = egui::Rect::from_min_max(
                egui::Pos2::new(
                    crosshair_rect.min.x,
                    crosshair_rect.center().y - line_width / 2.0,
                ),
                egui::Pos2::new(
                    crosshair_rect.max.x,
                    crosshair_rect.center().y + line_width / 2.0,
                ),
            );
            ui.painter()
                .rect_filled(line_rect, 1.0, egui::Color32::WHITE);

            // todo 这里也可以添加下方物品栏
        });
}

fn mian_ui(
    mut q: Query<
        (
            Entity,
            &'static mut EguiContext,
            Option<&'static PrimaryWindow>,
        ),
        With<Window>,
    >,
    user_textures: Res<EguiUserTextures>,
    ui_pic_resource_manager: Res<UiPicResourceManager>,
    mut tool_bar_data: ResMut<ToolBar>,
) {
    if let Ok((_, ctx, _)) = q.get_single_mut() {
        let bod_id = user_textures.image_id(&ui_pic_resource_manager.tool_box_border);
        egui::TopBottomPanel::bottom("tool_bar_bottom")
            .frame(frame_transparent())
            .resizable(false)
            .min_height(5.0)
            .show_separator_line(false)
            .show(ctx.into_inner().get_mut(), |ui| {
                ui.horizontal_centered(|ui| {
                    ui.vertical_centered_justified(|ui| {
                        tool_bar(
                            ui,
                            &mut tool_bar_data,
                            |image| user_textures.image_id(image),
                            bod_id,
                        );
                    });
                });
            });
    }
}

#[macro_export]
macro_rules! add_keyboard_toolbar {
    ($key: expr,$value: expr,$class: expr,$change:expr) => {
        if $class.just_pressed($key) {
            $change.active($value);
        }
    };
}

// 键盘控制 toolbar
fn controller_tool_bar(
    mut tool_bar_data: ResMut<ToolBar>,
    keyboard_input: Res<Input<KeyCode>>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
) {
    for event in mouse_wheel_events.iter() {
        // println!("{:?}", event);
        let y = event.y;
        if y > 0. {
            tool_bar_data.active_next();
        } else if y < 0. {
            tool_bar_data.active_pre();
        }
    }
    add_keyboard_toolbar!(KeyCode::Key1, 0, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key2, 1, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key3, 2, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key4, 3, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key5, 4, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key6, 5, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key7, 6, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key8, 7, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key9, 8, keyboard_input, tool_bar_data);
    add_keyboard_toolbar!(KeyCode::Key0, 9, keyboard_input, tool_bar_data);

    if keyboard_input.just_pressed(KeyCode::Right) {
        tool_bar_data.active_next();
    }
    if keyboard_input.just_pressed(KeyCode::Left) {
        tool_bar_data.active_pre();
    }
}

fn frame_transparent() -> egui::containers::Frame {
    egui::containers::Frame {
        inner_margin: egui::style::Margin {
            left: 10.,
            right: 10.,
            top: 10.,
            bottom: 10.,
        },
        outer_margin: egui::style::Margin {
            left: 10.,
            right: 10.,
            top: 10.,
            bottom: 10.,
        },
        rounding: egui::Rounding {
            nw: 1.0,
            ne: 1.0,
            sw: 1.0,
            se: 1.0,
        },
        shadow: Shadow {
            extrusion: 1.0,
            color: Color32::TRANSPARENT,
        },
        fill: Color32::TRANSPARENT,
        stroke: egui::Stroke::new(2.0, Color32::TRANSPARENT),
        // ..Default::default()
    }
}

fn disconnect_on_close(
    mut exit: EventWriter<AppExit>,
    mut closed: EventReader<WindowCloseRequested>,
    mut client: ResMut<RenetClient>,
) {
    for _ in closed.iter() {
        client.disconnect();
        exit.send(AppExit);
    }
}

fn chat_window(mut contexts: EguiContexts, mut input: ResMut<TextEditDemo>) {
    let ctx = contexts.ctx_mut();
    egui::Window::new("Chat")
        .title_bar(false)
        .vscroll(true)
        .resizable(false)
        .frame(egui::Frame::none().fill(egui::Color32::BLACK.gamma_multiply(0.8)))
        .default_height(200.0)
        .default_width(360.0)
        .anchor(egui::Align2::LEFT_BOTTOM, [0.0, 0.0])
        .collapsible(false)
        .show(ctx, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Player");
                    ui.label("time");
                    ui.colored_label(egui::Color32::RED, "text");
                });
            });

            egui::TopBottomPanel::bottom("bottom").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut input.input);

                    if ui.button("Send").clicked() {
                        //todo
                    };
                });
            })
        });
}
