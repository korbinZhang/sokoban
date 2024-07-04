use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    input::ButtonInput,
    log::LogPlugin,
    prelude::*,
    reflect::TypePath,
};
use bevy_utils::BoxedFuture;
use serde::Deserialize;

const BLOCK_SIZE: f32 = 30.;
const MAP_SIZE: usize = 20;
const GAME_WIDTH: f32 = BLOCK_SIZE * MAP_SIZE as f32 + BLOCK_SIZE;
const GAME_HEIGHT: f32 = BLOCK_SIZE * MAP_SIZE as f32 + BLOCK_SIZE;
const GAME_TRANSFORM_X: f32 = -GAME_WIDTH / 2.;
const GAME_TRANSFORM_Y: f32 = -GAME_HEIGHT / 2.;
const GAME_LEVEL_COUNT: usize = 50;
const INPUT_INTERVAL: f32 = 200.;

// block type
#[allow(dead_code)]
const BLOCK_TYPE_BLANK: usize = 0;
#[allow(dead_code)]
const BLOCK_TYPE_WALL: usize = 1;
const BLOCK_TYPE_GROUND: usize = 2;
const BLOCK_TYPE_BOX: usize = 3;
const BLOCK_TYPE_AIM: usize = 4;
const BLOCK_TYPE_PLAYER_DOWN: usize = 5;
const BLOCK_TYPE_PLAYER_RIGHT: usize = 6;
const BLOCK_TYPE_PLAYER_LEFT: usize = 7;
const BLOCK_TYPE_PLAYER_UP: usize = 8;
const BLOCK_TYPE_BOX_AIM: usize = 9;

#[derive(Component)]
struct MapBlock {
    x: usize,
    y: usize,
}

#[derive(Resource, Default)]
struct StepIntervalTimer(Timer);

#[derive(Resource)]
struct ImageHandles {
    textures: Vec<Handle<Image>>,
}

#[derive(Resource)]
struct SoundHandle {
    sound: Handle<AudioSource>,
}

#[derive(Resource)]
struct MapHandle {
    map: Handle<MapAsset>,
}

#[derive(Asset, TypePath, Debug, Deserialize)]
struct MapAsset {
    #[allow(dead_code)]
    value: [[usize; MAP_SIZE]; MAP_SIZE],
    position: Vec2,
}

#[derive(Default)]
struct MapAssetsLoader;

impl AssetLoader for MapAssetsLoader {
    type Asset = MapAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        _load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut value = [[0usize; MAP_SIZE]; MAP_SIZE];
            let mut position = Vec2::ZERO;
            let mut text = String::from_utf8(bytes).unwrap();
            debug!("{:?}", text);

            text = text.replace("\r\n", "\n");

            for i in 0..MAP_SIZE {
                for j in 0..MAP_SIZE {
                    let index = i * (MAP_SIZE + 1) + j;
                    value[j][MAP_SIZE - i - 1] =
                        (text[index..(index + 1)]).parse::<usize>().unwrap_or(0);
                    if value[j][MAP_SIZE - i - 1] == BLOCK_TYPE_PLAYER_DOWN {
                        position.x = j as f32;
                        position.y = (MAP_SIZE - i - 1) as f32;
                    }
                }
            }

            Ok(MapAsset { value, position })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["map"]
    }
}

#[derive(PartialEq)]
enum GameStatus {
    StartPlaying,
    Playing,
}

#[derive(Resource)]
struct Game {
    update: bool,
    level: usize,
    status: GameStatus,
    map: [[usize; MAP_SIZE]; MAP_SIZE],
    position: Vec2,
    position_type: usize,
    action: Option<KeyCode>,
}

impl Default for Game {
    fn default() -> Self {
        Game {
            update: true,
            level: 1,
            status: GameStatus::StartPlaying,
            map: [[0; MAP_SIZE]; MAP_SIZE],
            position: Vec2::new(0., 0.),
            position_type: BLOCK_TYPE_GROUND,
            action: None,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.build().disable::<LogPlugin>())
        .init_asset::<MapAsset>()
        .init_asset_loader::<MapAssetsLoader>()
        .add_systems(Startup, resource_setup)
        .add_systems(Update, game_update.after(resource_setup))
        .add_systems(Update, keyboard_input.after(resource_setup))
        .run();
}

fn resource_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    debug!("resource setup");
    commands.insert_resource(StepIntervalTimer(Timer::from_seconds(
        INPUT_INTERVAL / 1000.,
        TimerMode::Once,
    )));

    let mut textures = vec![];
    for i in 0..10 {
        textures.push(asset_server.load(format!("imgs/{}.png", i)));
    }
    textures.push(asset_server.load("imgs/backgroundImg.png"));
    textures.push(asset_server.load("imgs/toolImg.png"));
    commands.insert_resource(ImageHandles { textures });

    // map
    commands.insert_resource(MapHandle {
        map: asset_server.load("maps/1.map"),
    });

    // Sound
    let sound = asset_server.load("sounds/breakout_collision.ogg");
    commands.insert_resource(SoundHandle { sound });

    commands.insert_resource(Game::default());
    commands.spawn(Camera2dBundle::default());
}

fn game_update(
    mut commands: Commands,
    mut game: ResMut<Game>,
    imagehandles: Res<ImageHandles>,
    maphandle: Res<MapHandle>,
    asset_server: Res<AssetServer>,
    map_assets: Res<Assets<MapAsset>>,
    mut query: Query<(Entity, &MapBlock, &mut Handle<Image>)>,
) {
    if !game.update {
        return;
    }
    debug!("game update");
    game.update = false;
    match game.status {
        GameStatus::StartPlaying => {
            debug!("Start Playing");
            match map_assets.get(&maphandle.map) {
                Some(map) => {
                    debug!("load map {}.map success:", game.level);
                    game.map = map.value;
                    game.position = map.position;
                    for i in 0..MAP_SIZE {
                        for j in 0..MAP_SIZE {
                            let x = (i as f32) * BLOCK_SIZE + GAME_TRANSFORM_X;
                            let y = (j as f32) * BLOCK_SIZE + GAME_TRANSFORM_Y;
                            commands
                                .spawn(SpriteBundle {
                                    texture: imagehandles.textures[game.map[i][j]].clone(),
                                    transform: Transform {
                                        translation: Vec3::new(x, y, 0.),
                                        ..default()
                                    },
                                    ..default()
                                })
                                .insert(MapBlock { x: i, y: j });
                        }
                    }
                    game.status = GameStatus::Playing;
                }
                _ => {
                    debug!("load map {}.map error", game.level);
                    game.update = true;
                }
            }
        }
        GameStatus::Playing => {
            debug!("Playing Game");
            if !game.win() {
                for (_, mapblock, mut imagehandle) in query.iter_mut() {
                    if *imagehandle != imagehandles.textures[game.map[mapblock.x][mapblock.y]] {
                        *imagehandle =
                            imagehandles.textures[game.map[mapblock.x][mapblock.y]].clone();
                    }
                }
            } else {
                commands.insert_resource(MapHandle {
                    map: asset_server.load(format!("maps/{}.map", game.level)),
                });
                for (entity, _, _) in query.iter_mut() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

fn keyboard_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut timer: ResMut<StepIntervalTimer>,
    mut game: ResMut<Game>,
    mut commands: Commands,
    sound: Res<SoundHandle>,
) {
    if !timer.0.tick(time.delta()).finished() {
        return;
    }
    if keyboard_input.any_pressed([
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
    ]) {
        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            game.action = Some(KeyCode::ArrowLeft);
            timer.0.reset();
        }
        if keyboard_input.pressed(KeyCode::ArrowRight) {
            game.action = Some(KeyCode::ArrowRight);
            timer.0.reset();
        }
        if keyboard_input.pressed(KeyCode::ArrowUp) {
            game.action = Some(KeyCode::ArrowUp);
            timer.0.reset();
        }
        if keyboard_input.pressed(KeyCode::ArrowDown) {
            game.action = Some(KeyCode::ArrowDown);
            timer.0.reset();
        }
        commands.spawn(AudioBundle {
            source: sound.sound.clone(),
            settings: PlaybackSettings::DESPAWN,
        });
        game.update();
    }
}

impl Game {
    const fn get_player_type(action: Vec2) -> usize {
        match (action.x as i32, action.y as i32) {
            (1, 0) => BLOCK_TYPE_PLAYER_LEFT,
            (-1, 0) => BLOCK_TYPE_PLAYER_RIGHT,
            (0, 1) => BLOCK_TYPE_PLAYER_UP,
            (0, -1) => BLOCK_TYPE_PLAYER_DOWN,
            _ => BLOCK_TYPE_PLAYER_UP,
        }
    }
    fn step(&mut self, action: Vec2) {
        let next_position = self.position + action;
        debug!(
            "position:{},actioin:{},next_position:{}",
            self.position, action, next_position
        );
        if next_position.x.clamp(0., (MAP_SIZE - 1) as f32) != next_position.x
            || next_position.y.clamp(0., (MAP_SIZE - 1) as f32) != next_position.y
        {
            return;
        }
        // P _ -> X P,_
        // P A -> X P,A
        if self.map[next_position.x as usize][next_position.y as usize] == BLOCK_TYPE_GROUND
            || self.map[next_position.x as usize][next_position.y as usize] == BLOCK_TYPE_AIM
        {
            self.map[self.position.x as usize][self.position.y as usize] = self.position_type;
            self.position_type = self.map[next_position.x as usize][next_position.y as usize];
            self.map[next_position.x as usize][next_position.y as usize] =
                Self::get_player_type(action);
            self.position = next_position;
            self.update = true;
        }
        // P B _ -> X P,_ B
        // P B A -> X P,_ W
        // P W _ -> X P,A B
        // P W A -> X P,A W
        if self.map[next_position.x as usize][next_position.y as usize] == BLOCK_TYPE_BOX
            || self.map[next_position.x as usize][next_position.y as usize] == BLOCK_TYPE_BOX_AIM
        {
            let next2_position = next_position + action;
            if next2_position.x.clamp(0., (MAP_SIZE - 1) as f32) != next2_position.x
                || next2_position.y.clamp(0., (MAP_SIZE - 1) as f32) != next2_position.y
            {
                return;
            }
            if self.map[next2_position.x as usize][next2_position.y as usize] == BLOCK_TYPE_GROUND
                || self.map[next2_position.x as usize][next2_position.y as usize] == BLOCK_TYPE_AIM
            {
                self.map[self.position.x as usize][self.position.y as usize] = self.position_type;
                if self.map[next_position.x as usize][next_position.y as usize] == BLOCK_TYPE_BOX {
                    self.position_type = BLOCK_TYPE_GROUND;
                } else {
                    self.position_type = BLOCK_TYPE_AIM;
                }
                self.map[next_position.x as usize][next_position.y as usize] =
                    Self::get_player_type(action);
                if self.map[next2_position.x as usize][next2_position.y as usize]
                    == BLOCK_TYPE_GROUND
                {
                    self.map[next2_position.x as usize][next2_position.y as usize] = BLOCK_TYPE_BOX;
                } else {
                    self.map[next2_position.x as usize][next2_position.y as usize] =
                        BLOCK_TYPE_BOX_AIM;
                }
                self.position = next_position;
                self.update = true;
            }
        }
    }
    fn win(&mut self) -> bool {
        for i in 0..MAP_SIZE {
            for j in 0..MAP_SIZE {
                if self.map[i][j] == BLOCK_TYPE_BOX {
                    return false;
                }
            }
        }
        self.level = self.level + 1;
        if self.level > GAME_LEVEL_COUNT {
            self.level = 1;
        }
        self.update = true;
        self.status = GameStatus::StartPlaying;
        return true;
    }
    fn update(&mut self) {
        if self.status != GameStatus::Playing {
            return;
        }
        if let Some(action) = self.action {
            match action {
                KeyCode::ArrowLeft => {
                    self.step(Vec2::new(-1., 0.));
                }
                KeyCode::ArrowRight => {
                    self.step(Vec2::new(1., 0.));
                }
                KeyCode::ArrowUp => {
                    self.step(Vec2::new(0., 1.));
                }
                KeyCode::ArrowDown => {
                    self.step(Vec2::new(0., -1.));
                }
                _ => {}
            }
        }
        self.action = None;
    }
}
