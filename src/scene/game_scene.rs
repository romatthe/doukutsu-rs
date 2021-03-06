use itertools::Itertools;
use log::info;

use crate::bullet::BulletManager;
use crate::caret::CaretType;
use crate::common::{Direction, FadeDirection, FadeState, fix9_scale, Rect};
use crate::components::boss_life_bar::BossLifeBar;
use crate::entity::GameEntity;
use crate::frame::{Frame, UpdateTarget};
use crate::ggez::{Context, GameResult, graphics, timer};
use crate::ggez::graphics::{BlendMode, Color, Drawable, DrawParam, FilterMode, Vector2};
use crate::ggez::nalgebra::clamp;
use crate::inventory::{Inventory, TakeExperienceResult};
use crate::npc::NPCMap;
use crate::physics::PhysicalEntity;
use crate::player::{Player, PlayerAppearance};
use crate::rng::RNG;
use crate::scene::Scene;
use crate::scene::title_scene::TitleScene;
use crate::shared_game_state::{Season, SharedGameState};
use crate::stage::{BackgroundType, Stage};
use crate::text_script::{ConfirmSelection, ScriptMode, TextScriptExecutionState, TextScriptVM};
use crate::texture_set::SizedBatch;
use crate::ui::Components;
use crate::weapon::WeaponType;

pub struct GameScene {
    pub tick: usize,
    pub stage: Stage,
    pub boss_life_bar: BossLifeBar,
    pub frame: Frame,
    pub player: Player,
    pub inventory: Inventory,
    pub stage_id: usize,
    pub npc_map: NPCMap,
    pub bullet_manager: BulletManager,
    pub current_teleport_slot: u8,
    pub intro_mode: bool,
    tex_background_name: String,
    tex_tileset_name: String,
    life_bar: u16,
    life_bar_counter: u16,
    map_name_counter: u16,
    stage_select_text_y_pos: usize,
    weapon_x_pos: isize,
}

#[derive(Debug, EnumIter, PartialEq, Eq, Hash, Copy, Clone)]
pub enum TileLayer {
    All,
    Background,
    Foreground,
    Snack,
}

#[derive(Debug, EnumIter, PartialEq, Eq, Hash, Copy, Clone)]
pub enum Alignment {
    Left,
    Right,
}

static FACE_TEX: &str = "Face";
static SWITCH_FACE_TEX: [&str; 4] = ["Face1", "Face2", "Face3", "Face4"];

impl GameScene {
    pub fn new(state: &mut SharedGameState, ctx: &mut Context, id: usize) -> GameResult<Self> {
        info!("Loading stage {} ({})", id, &state.stages[id].map);
        let stage = Stage::load(&state.base_path, &state.stages[id], ctx)?;
        info!("Loaded stage: {}", stage.data.name);

        let tex_background_name = stage.data.background.filename();
        let tex_tileset_name = ["Stage/", &stage.data.tileset.filename()].join("");

        Ok(Self {
            tick: 0,
            stage,
            player: Player::new(state),
            inventory: Inventory::new(),
            boss_life_bar: BossLifeBar::new(),
            frame: Frame {
                x: 0,
                y: 0,
                prev_x: 0,
                prev_y: 0,
                update_target: UpdateTarget::Player,
                target_x: 0,
                target_y: 0,
                wait: 16,
            },
            stage_id: id,
            npc_map: NPCMap::new(),
            bullet_manager: BulletManager::new(),
            current_teleport_slot: 0,
            intro_mode: false,
            tex_background_name,
            tex_tileset_name,
            life_bar: 0,
            life_bar_counter: 0,
            map_name_counter: 0,
            stage_select_text_y_pos: 54,
            weapon_x_pos: 16,
        })
    }

    pub fn display_map_name(&mut self, ticks: u16) {
        self.map_name_counter = ticks;
    }

    fn draw_number(&self, x: f32, y: f32, val: usize, align: Alignment, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "TextBox")?;
        let n = val.to_string();
        let align_offset = if align == Alignment::Right { n.len() as f32 * 8.0 } else { 0.0 };

        for (offset, chr) in n.chars().enumerate() {
            let idx = chr as usize - '0' as usize;
            batch.add_rect(x - align_offset + offset as f32 * 8.0, y, &Rect::<usize>::new_size(idx * 8, 56, 8, 8));
        }

        batch.draw(ctx)?;
        Ok(())
    }

    fn draw_hud(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        // none
        let weap_x = self.weapon_x_pos as f32;
        let (ammo, max_ammo) = self.inventory.get_current_ammo();
        let (xp, max_xp, max_level) = self.inventory.get_current_max_exp(&state.constants);
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "TextBox")?;

        if max_ammo == 0 {
            batch.add_rect(weap_x + 48.0, 16.0,
                           &Rect::<usize>::new_size(80, 48, 16, 8));
            batch.add_rect(weap_x + 48.0, 24.0,
                           &Rect::<usize>::new_size(80, 48, 16, 8));
        }

        // per
        batch.add_rect(weap_x + 32.0, 24.0,
                       &Rect::<usize>::new_size(72, 48, 8, 8));
        // lv
        batch.add_rect(weap_x, 32.0,
                       &Rect::<usize>::new_size(80, 80, 16, 8));
        // xp box
        batch.add_rect(weap_x + 24.0, 32.0,
                       &Rect::<usize>::new_size(0, 72, 40, 8));

        if max_level {
            batch.add_rect(weap_x + 24.0, 32.0,
                           &Rect::<usize>::new_size(40, 72, 40, 8));
        } else if max_xp > 0 {
            // xp bar
            let bar_width = (xp as f32 / max_xp as f32 * 40.0) as usize;

            batch.add_rect(weap_x + 24.0, 32.0,
                           &Rect::<usize>::new_size(0, 80, bar_width, 8));
        }

        if self.player.max_life != 0 {
            // life box
            batch.add_rect(16.0, 40.0,
                           &Rect::new_size(0, 40, 64, 8));
            // yellow bar
            batch.add_rect(40.0, 40.0,
                           &Rect::new_size(0, 32, ((self.life_bar as usize * 40) / self.player.max_life as usize), 8));
            // life
            batch.add_rect(40.0, 40.0,
                           &Rect::new_size(0, 24, ((self.player.life as usize * 40) / self.player.max_life as usize), 8));
        }

        if self.player.air_counter > 0 {
            let rect = if self.player.air % 30 > 10 {
                Rect::new_size(112, 72, 32, 8)
            } else {
                Rect::new_size(112, 80, 32, 8)
            };

            batch.add_rect((state.canvas_size.0 / 2.0).floor() - 40.0,
                           (state.canvas_size.1 / 2.0).floor(), &rect);
        }

        batch.draw(ctx)?;
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "ArmsImage")?;

        let weapon_count = self.inventory.get_weapon_count();
        if weapon_count != 0 {
            let current_weapon = self.inventory.get_current_weapon_idx() as isize;
            let mut rect = Rect::new(0, 0, 0, 16);

            for a in 0..weapon_count {
                let mut pos_x = ((a as isize - current_weapon) as f32 * 16.0) + weap_x;

                if pos_x < 8.0 {
                    pos_x += 48.0 + weapon_count as f32 * 16.0;
                } else if pos_x >= 24.0 {
                    pos_x += 48.0;
                }

                if pos_x >= 72.0 + ((weapon_count - 1) as f32 * 16.0) {
                    pos_x -= 48.0 + weapon_count as f32 * 16.0;
                } else if pos_x < 72.0 && pos_x >= 24.0 {
                    pos_x -= 48.0;
                }

                if let Some(weapon) = self.inventory.get_weapon(a) {
                    rect.left = weapon.wtype as usize * 16;
                    rect.right = rect.left + 16;
                    batch.add_rect(pos_x, 16.0, &rect);
                }
            }
        }

        batch.draw(ctx)?;

        if self.player.air_counter > 0 && self.player.air_counter % 6 < 4 {
            self.draw_number((state.canvas_size.0 / 2.0).floor() + 8.0,
                             (state.canvas_size.1 / 2.0).floor(),
                             (self.player.air / 10) as usize, Alignment::Left, state, ctx)?;
        }

        if max_ammo != 0 {
            self.draw_number(weap_x + 64.0, 16.0, ammo as usize, Alignment::Right, state, ctx)?;
            self.draw_number(weap_x + 64.0, 24.0, max_ammo as usize, Alignment::Right, state, ctx)?;
        }
        self.draw_number(weap_x + 24.0, 32.0, self.inventory.get_current_level() as usize, Alignment::Right, state, ctx)?;
        self.draw_number(40.0, 40.0, self.life_bar as usize, Alignment::Right, state, ctx)?;

        Ok(())
    }

    fn draw_background(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, &self.tex_background_name)?;
        let scale = state.scale;
        let (frame_x, frame_y) = self.frame.xy_interpolated(state.frame_time, state.scale);

        match self.stage.data.background_type {
            BackgroundType::Stationary => {
                let count_x = state.canvas_size.0 as usize / batch.width() + 1;
                let count_y = state.canvas_size.1 as usize / batch.height() + 1;

                for y in 0..count_y {
                    for x in 0..count_x {
                        batch.add((x * batch.width()) as f32, (y * batch.height()) as f32);
                    }
                }
            }
            BackgroundType::MoveDistant | BackgroundType::MoveNear => {
                let (off_x, off_y) = if self.stage.data.background_type == BackgroundType::MoveNear {
                    (
                        frame_x % (batch.width() as f32),
                        frame_y % (batch.height() as f32)
                    )
                } else {
                    (
                        ((frame_x / 2.0 * scale).floor() / scale) % (batch.width() as f32),
                        ((frame_y / 2.0 * scale).floor() / scale) % (batch.height() as f32)
                    )
                };

                let count_x = state.canvas_size.0 as usize / batch.width() + 2;
                let count_y = state.canvas_size.1 as usize / batch.height() + 2;

                for y in 0..count_y {
                    for x in 0..count_x {
                        batch.add((x * batch.width()) as f32 - off_x,
                                  (y * batch.height()) as f32 - off_y);
                    }
                }
            }
            BackgroundType::Water => {}
            BackgroundType::Black => {
                graphics::clear(ctx, Color::from_rgb(0, 0, 32));
            }
            BackgroundType::Autoscroll => {}
            BackgroundType::OutsideWind | BackgroundType::Outside => {
                let offset = (self.tick % 640) as isize;

                batch.add_rect(((state.canvas_size.0 - 320.0) / 2.0).floor(), 0.0,
                               &Rect::<usize>::new_size(0, 0, 320, 88));

                for x in ((-offset / 2)..(state.canvas_size.0 as isize)).step_by(320) {
                    batch.add_rect(x as f32, 88.0,
                                   &Rect::<usize>::new_size(0, 88, 320, 35));
                }

                for x in ((-offset % 320)..(state.canvas_size.0 as isize)).step_by(320) {
                    batch.add_rect(x as f32, 123.0,
                                   &Rect::<usize>::new_size(0, 123, 320, 23));
                }

                for x in ((-offset * 2)..(state.canvas_size.0 as isize)).step_by(320) {
                    batch.add_rect(x as f32, 146.0,
                                   &Rect::<usize>::new_size(0, 146, 320, 30));
                }

                for x in ((-offset * 4)..(state.canvas_size.0 as isize)).step_by(320) {
                    batch.add_rect(x as f32, 176.0,
                                   &Rect::<usize>::new_size(0, 176, 320, 64));
                }
            }
        }

        batch.draw(ctx)?;

        Ok(())
    }

    fn draw_bullets(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "Bullet")?;
        let scale = state.scale;
        let mut x: isize;
        let mut y: isize;

        for bullet in self.bullet_manager.bullets.iter() {
            match bullet.direction {
                Direction::Left => {
                    x = bullet.x - bullet.display_bounds.left as isize;
                    y = bullet.y - bullet.display_bounds.top as isize;
                }
                Direction::Up => {
                    x = bullet.x - bullet.display_bounds.top as isize;
                    y = bullet.y - bullet.display_bounds.left as isize;
                }
                Direction::Right => {
                    x = bullet.x - bullet.display_bounds.right as isize;
                    y = bullet.y - bullet.display_bounds.top as isize;
                }
                Direction::Bottom => {
                    x = bullet.x - bullet.display_bounds.top as isize;
                    y = bullet.y - bullet.display_bounds.right as isize;
                }
                Direction::FacingPlayer => unreachable!(),
            }

            batch.add_rect(fix9_scale(x - self.frame.x, scale),
                           fix9_scale(y - self.frame.y, scale),
                           &bullet.anim_rect);
        }

        batch.draw(ctx)?;
        Ok(())
    }

    fn draw_carets(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "Caret")?;
        let scale = state.scale;

        for caret in state.carets.iter() {
            batch.add_rect(fix9_scale(caret.x - caret.offset_x - self.frame.x, scale),
                           fix9_scale(caret.y - caret.offset_y - self.frame.y, scale),
                           &caret.anim_rect);
        }

        batch.draw(ctx)?;
        Ok(())
    }

    fn draw_fade(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        match state.fade_state {
            FadeState::Visible => { return Ok(()); }
            FadeState::Hidden => {
                graphics::clear(ctx, Color::from_rgb(0, 0, 32));
            }
            FadeState::FadeIn(tick, direction) | FadeState::FadeOut(tick, direction) => {
                let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "Fade")?;
                let mut rect = Rect::<usize>::new(0, 0, 16, 16);

                match direction {
                    FadeDirection::Left | FadeDirection::Right => {
                        let mut frame = tick;

                        for x in (0..(state.canvas_size.0 as isize + 16)).step_by(16) {
                            if frame > 15 { frame = 15; } else { frame += 1; }

                            if frame >= 0 {
                                rect.left = frame as usize * 16;
                                rect.right = rect.left + 16;

                                for y in (0..(state.canvas_size.1 as isize + 16)).step_by(16) {
                                    if direction == FadeDirection::Left {
                                        batch.add_rect(state.canvas_size.0 - x as f32, y as f32, &rect);
                                    } else {
                                        batch.add_rect(x as f32, y as f32, &rect);
                                    }
                                }
                            }
                        }
                    }
                    FadeDirection::Up | FadeDirection::Down => {
                        let mut frame = tick;

                        for y in (0..(state.canvas_size.1 as isize + 16)).step_by(16) {
                            if frame > 15 { frame = 15; } else { frame += 1; }

                            if frame >= 0 {
                                rect.left = frame as usize * 16;
                                rect.right = rect.left + 16;

                                for x in (0..(state.canvas_size.0 as isize + 16)).step_by(16) {
                                    if direction == FadeDirection::Down {
                                        batch.add_rect(x as f32, y as f32, &rect);
                                    } else {
                                        batch.add_rect(x as f32, state.canvas_size.1 - y as f32, &rect);
                                    }
                                }
                            }
                        }
                    }
                    FadeDirection::Center => {
                        let center_x = (state.canvas_size.0 / 2.0 - 8.0) as isize;
                        let center_y = (state.canvas_size.1 / 2.0 - 8.0) as isize;
                        let mut start_frame = tick;

                        for x in (0..(center_x + 16)).step_by(16) {
                            let mut frame = start_frame;

                            for y in (0..(center_y + 16)).step_by(16) {
                                if frame > 15 { frame = 15; } else { frame += 1; }

                                if frame >= 0 {
                                    rect.left = frame as usize * 16;
                                    rect.right = rect.left + 16;

                                    batch.add_rect((center_x - x) as f32, (center_y + y) as f32, &rect);
                                    batch.add_rect((center_x - x) as f32, (center_y - y) as f32, &rect);
                                    batch.add_rect((center_x + x) as f32, (center_y + y) as f32, &rect);
                                    batch.add_rect((center_x + x) as f32, (center_y - y) as f32, &rect);
                                }
                            }

                            start_frame += 1;
                        }
                    }
                }

                batch.draw(ctx)?;
            }
        }

        Ok(())
    }

    fn draw_black_bars(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        Ok(())
    }

    fn draw_text_boxes(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        if !state.textscript_vm.flags.render() { return Ok(()); }

        let top_pos = if state.textscript_vm.flags.position_top() { 32.0 } else { state.canvas_size.1 as f32 - 66.0 };
        let left_pos = (state.canvas_size.0 / 2.0 - 122.0).floor();

        {
            let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "TextBox")?;
            if state.textscript_vm.flags.background_visible() {
                batch.add_rect(left_pos, top_pos, &state.constants.textscript.textbox_rect_top);
                for i in 1..7 {
                    batch.add_rect(left_pos, top_pos + i as f32 * 8.0, &state.constants.textscript.textbox_rect_middle);
                }
                batch.add_rect(left_pos, top_pos + 56.0, &state.constants.textscript.textbox_rect_bottom);
            }

            if state.textscript_vm.item != 0 {
                batch.add_rect((state.canvas_size.0 / 2.0 - 40.0).floor(), state.canvas_size.1 - 112.0,
                               &state.constants.textscript.get_item_top_left);
                batch.add_rect((state.canvas_size.0 / 2.0 - 40.0).floor(), state.canvas_size.1 - 96.0,
                               &state.constants.textscript.get_item_bottom_left);
                batch.add_rect((state.canvas_size.0 / 2.0 + 32.0).floor(), state.canvas_size.1 - 112.0,
                               &state.constants.textscript.get_item_top_right);
                batch.add_rect((state.canvas_size.0 / 2.0 + 32.0).floor(), state.canvas_size.1 - 104.0,
                               &state.constants.textscript.get_item_right);
                batch.add_rect((state.canvas_size.0 / 2.0 + 32.0).floor(), state.canvas_size.1 - 96.0,
                               &state.constants.textscript.get_item_right);
                batch.add_rect((state.canvas_size.0 / 2.0 + 32.0).floor(), state.canvas_size.1 - 88.0,
                               &state.constants.textscript.get_item_bottom_right);
            }

            if let TextScriptExecutionState::WaitConfirmation(_, _, _, wait, selection) = state.textscript_vm.state {
                let pos_y = if wait > 14 {
                    state.canvas_size.1 - 96.0 - (wait as f32 - 2.0) * 4.0
                } else {
                    state.canvas_size.1 - 96.0
                };

                batch.add_rect((state.canvas_size.0 / 2.0 + 56.0).floor(), pos_y,
                               &state.constants.textscript.textbox_rect_yes_no);

                if wait == 0 {
                    let pos_x = if selection == ConfirmSelection::No { 41.0 } else { 0.0 };

                    batch.add_rect((state.canvas_size.0 / 2.0 + 51.0).floor() + pos_x,
                                   state.canvas_size.1 - 86.0,
                                   &state.constants.textscript.textbox_rect_cursor);
                }
            }

            batch.draw(ctx)?;
        }

        if state.textscript_vm.face != 0 {
            let tex_name = if state.constants.textscript.animated_face_pics {
                SWITCH_FACE_TEX[0]
            } else {
                FACE_TEX
            };
            let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, tex_name)?;

            // switch version uses +1000 face offset to display a flipped version
            let flip = state.textscript_vm.face > 1000;
            let face_num = state.textscript_vm.face % 100;
            let (scale_x, scale_y) = batch.scale();

            batch.add_rect_scaled(left_pos + 14.0 + if flip { 48.0 } else { 0.0 }, top_pos + 8.0,
                                  scale_x * if flip { -1.0 } else { 1.0 }, scale_y,
                                  &Rect::<usize>::new_size(
                                      (face_num as usize % 6) * 48,
                                      (face_num as usize / 6) * 48,
                                      48, 48,
                                  ));

            batch.draw(ctx)?;
        }

        if state.textscript_vm.item != 0 {
            let mut rect = Rect::<usize>::new(0, 0, 0, 0);

            if state.textscript_vm.item < 1000 {
                let item_id = state.textscript_vm.item as usize;

                rect.left = (item_id % 16) * 16;
                rect.right = rect.left + 16;
                rect.top = (item_id / 16) * 16;
                rect.bottom = rect.top + 16;

                let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "ArmsImage")?;
                batch.add_rect((state.canvas_size.0 / 2.0 - 12.0).floor(), state.canvas_size.1 - 104.0, &rect);
                batch.draw(ctx)?;
            } else {
                let item_id = state.textscript_vm.item as usize - 1000;

                rect.left = (item_id % 8) * 32;
                rect.right = rect.left + 32;
                rect.top = (item_id / 8) * 16;
                rect.bottom = rect.top + 16;

                let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "ItemImage")?;
                batch.add_rect((state.canvas_size.0 / 2.0 - 20.0).floor(), state.canvas_size.1 - 104.0, &rect);
                batch.draw(ctx)?;
            }
        }

        let text_offset = if state.textscript_vm.face == 0 { 0.0 } else { 56.0 };

        if !state.textscript_vm.line_1.is_empty() {
            state.font.draw_text(state.textscript_vm.line_1.iter().copied(), left_pos + text_offset + 14.0, top_pos + 10.0, &state.constants, &mut state.texture_set, ctx)?;
        }

        if !state.textscript_vm.line_2.is_empty() {
            state.font.draw_text(state.textscript_vm.line_2.iter().copied(), left_pos + text_offset + 14.0, top_pos + 10.0 + 16.0, &state.constants, &mut state.texture_set, ctx)?;
        }

        if !state.textscript_vm.line_3.is_empty() {
            state.font.draw_text(state.textscript_vm.line_3.iter().copied(), left_pos + text_offset + 14.0, top_pos + 10.0 + 32.0, &state.constants, &mut state.texture_set, ctx)?;
        }

        Ok(())
    }

    fn draw_light(&self, x: f32, y: f32, size: f32, color: (u8, u8, u8), batch: &mut SizedBatch) {
        batch.add_rect_scaled_tinted(x - size * 32.0, y - size * 32.0, color,
                                     size,
                                     size,
                                     &Rect::new(0, 0, 64, 64))
    }

    fn draw_light_map(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        graphics::set_canvas(ctx, Some(&state.lightmap_canvas));
        graphics::set_blend_mode(ctx, BlendMode::Add)?;

        graphics::clear(ctx, Color::from_rgb(100, 100, 110));
        {
            let scale = state.scale;
            let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "builtin/lightmap/spot")?;

            if !self.player.cond.hidden() && self.inventory.get_current_weapon().is_some() {
                self.draw_light(fix9_scale(self.player.x - self.frame.x, scale),
                                fix9_scale(self.player.y - self.frame.y, scale),
                                2.5, (225, 225, 225), batch);
            }

            for bullet in self.bullet_manager.bullets.iter() {
                self.draw_light(fix9_scale(bullet.x - self.frame.x, scale),
                                fix9_scale(bullet.y - self.frame.y, scale),
                                0.7, (200, 200, 200), batch);
            }

            for caret in state.carets.iter() {
                match caret.ctype {
                    CaretType::ProjectileDissipation | CaretType::Shoot => {
                        self.draw_light(fix9_scale(caret.x - self.frame.x, scale),
                                        fix9_scale(caret.y - self.frame.y, scale),
                                        1.0, (200, 200, 200), batch);
                    }
                    _ => {}
                }
            }

            for npc_cell in self.npc_map.npcs.values() {
                let npc = npc_cell.borrow();

                if npc.x < (self.frame.x - 128 - npc.display_bounds.width() as isize * 0x200)
                    || npc.x > (self.frame.x + (state.canvas_size.0 as isize + npc.display_bounds.width() as isize) * 0x200)
                    && npc.y < (self.frame.y - 128 - npc.display_bounds.height() as isize * 0x200)
                    || npc.y > (self.frame.y + (state.canvas_size.1 as isize + npc.display_bounds.height() as isize) * 0x200) {
                    continue;
                }

                match npc.npc_type {
                    1 => {
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        0.4, (255, 255, 0), batch);
                    }
                    4 | 7 => self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                             fix9_scale(npc.y - self.frame.y, scale),
                                             1.0, (100, 100, 100), batch),
                    17 if npc.anim_num == 0 => {
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        2.0, (160, 0, 0), batch);
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        0.5, (255, 0, 0), batch);
                    }
                    20 if npc.direction == Direction::Right => {
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        2.0, (0, 0, 150), batch);

                        if npc.anim_num < 2 {
                            self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                            fix9_scale(npc.y - self.frame.y, scale),
                                            2.1, (0, 0, 30), batch);
                        }
                    }
                    22 if npc.action_num == 1 && npc.anim_num == 1 =>
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        3.0, (0, 0, 255), batch),
                    32 | 87 | 211 => {
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        2.0, (255, 30, 30), batch);
                    }
                    38 => {
                        let flicker = (npc.anim_num ^ 5 & 3) as u8 * 15;
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        3.5, (130 + flicker, 40 + flicker, 0), batch);
                    }
                    66 if npc.action_num == 1 && npc.anim_counter % 2 == 0 =>
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        3.0, (0, 100, 255), batch),
                    67 => self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                          fix9_scale(npc.y - self.frame.y, scale),
                                          2.0, (0, 100, 200), batch),
                    70 => {
                        let flicker = 50 + npc.anim_num as u8 * 15;
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        2.0, (flicker, flicker, flicker), batch);
                    }
                    75 | 77 => self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                               fix9_scale(npc.y - self.frame.y, scale),
                                               3.0, (255, 100, 0), batch),
                    85 if npc.action_num == 1 => {
                        let (color, color2) = if npc.direction == Direction::Left {
                            ((0, 150, 100), (0, 50, 30))
                        } else {
                            ((150, 0, 0), (50, 0, 0))
                        };

                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        1.5, color, batch);

                        if npc.anim_num < 2 {
                            self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                            fix9_scale(npc.y - self.frame.y, scale) - 8.0,
                                            2.1, color2, batch);
                        }
                    }
                    299 => {
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        4.0, (30, 30, 200), batch);
                    }
                    300 => {
                        self.draw_light(fix9_scale(npc.x - self.frame.x, scale),
                                        fix9_scale(npc.y - self.frame.y, scale),
                                        1.5, (200, 10, 10), batch);
                    }
                    _ => {}
                }
            }

            batch.draw_filtered(FilterMode::Linear, ctx)?;
        }

        graphics::set_canvas(ctx, None);
        graphics::set_blend_mode(ctx, BlendMode::Multiply)?;
        state.lightmap_canvas.set_filter(FilterMode::Linear);
        state.lightmap_canvas.draw(ctx, DrawParam::new()
            .scale(Vector2::new(1.0 / state.scale, 1.0 / state.scale)))?;

        graphics::set_blend_mode(ctx, BlendMode::Alpha)?;

        Ok(())
    }

    fn draw_tiles(&self, state: &mut SharedGameState, ctx: &mut Context, layer: TileLayer) -> GameResult {
        let tex = match layer {
            TileLayer::Snack => "Npc/NpcSym",
            _ => &self.tex_tileset_name,
        };
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, tex)?;
        let mut rect = Rect::<usize>::new(0, 0, 16, 16);
        let (frame_x, frame_y) = self.frame.xy_interpolated(state.frame_time, state.scale);

        let tile_start_x = clamp(self.frame.x / 0x200 / 16, 0, self.stage.map.width as isize) as usize;
        let tile_start_y = clamp(self.frame.y / 0x200 / 16, 0, self.stage.map.height as isize) as usize;
        let tile_end_x = clamp((self.frame.x / 0x200 + 8 + state.canvas_size.0 as isize) / 16 + 1, 0, self.stage.map.width as isize) as usize;
        let tile_end_y = clamp((self.frame.y / 0x200 + 8 + state.canvas_size.1 as isize) / 16 + 1, 0, self.stage.map.height as isize) as usize;

        if layer == TileLayer::Snack {
            rect = state.constants.world.snack_rect;
        }

        for y in tile_start_y..tile_end_y {
            for x in tile_start_x..tile_end_x {
                let tile = *self.stage.map.tiles
                    .get((y * self.stage.map.width) + x)
                    .unwrap();

                match layer {
                    TileLayer::Background => {
                        if self.stage.map.attrib[tile as usize] >= 0x20 {
                            continue;
                        }

                        rect.left = (tile as usize % 16) * 16;
                        rect.top = (tile as usize / 16) * 16;
                        rect.right = rect.left + 16;
                        rect.bottom = rect.top + 16;
                    }
                    TileLayer::Foreground => {
                        let attr = self.stage.map.attrib[tile as usize];

                        if attr < 0x40 || attr >= 0x80 || attr == 0x43 {
                            continue;
                        }

                        rect.left = (tile as usize % 16) * 16;
                        rect.top = (tile as usize / 16) * 16;
                        rect.right = rect.left + 16;
                        rect.bottom = rect.top + 16;
                    }
                    TileLayer::Snack => {
                        if self.stage.map.attrib[tile as usize] != 0x43 {
                            continue;
                        }
                    }
                    _ => {}
                }

                batch.add_rect((x as f32 * 16.0 - 8.0) - frame_x,
                               (y as f32 * 16.0 - 8.0) - frame_y, &rect);
            }
        }

        batch.draw(ctx)?;

        Ok(())
    }

    fn tick_npc_bullet_collissions(&mut self, state: &mut SharedGameState) {
        let mut dead_npcs = Vec::new();

        for npc_id in self.npc_map.npc_ids.iter() {
            if let Some(npc_cell) = self.npc_map.npcs.get(npc_id) {
                let mut npc = npc_cell.borrow_mut();

                if npc.cond.drs_destroyed() {
                    dead_npcs.push(npc.id);
                }

                if !npc.cond.alive() {
                    continue;
                }

                if npc.npc_flags.shootable() && npc.npc_flags.interactable() {
                    continue;
                }

                for bullet in self.bullet_manager.bullets.iter_mut() {
                    if !bullet.cond.alive() || bullet.damage < 1 {
                        continue;
                    }

                    let hit = (
                        npc.npc_flags.shootable()
                            && (npc.x - npc.hit_bounds.right as isize) < (bullet.x + bullet.enemy_hit_width as isize)
                            && (npc.x + npc.hit_bounds.right as isize) > (bullet.x - bullet.enemy_hit_width as isize)
                            && (npc.y - npc.hit_bounds.top as isize) < (bullet.y + bullet.enemy_hit_height as isize)
                            && (npc.y + npc.hit_bounds.bottom as isize) > (bullet.y - bullet.enemy_hit_height as isize)
                    ) || (
                        npc.npc_flags.invulnerable()
                            && (npc.x - npc.hit_bounds.right as isize) < (bullet.x + bullet.hit_bounds.right as isize)
                            && (npc.x + npc.hit_bounds.right as isize) > (bullet.x - bullet.hit_bounds.left as isize)
                            && (npc.y - npc.hit_bounds.top as isize) < (bullet.y + bullet.hit_bounds.bottom as isize)
                            && (npc.y + npc.hit_bounds.bottom as isize) > (bullet.y - bullet.hit_bounds.top as isize)
                    );

                    if !hit {
                        continue;
                    }

                    if npc.npc_flags.shootable() {
                        npc.life = npc.life.saturating_sub(bullet.damage);

                        if npc.life == 0 {
                            if npc.npc_flags.show_damage() {
                                // todo show damage
                            }

                            if self.player.cond.alive() && npc.npc_flags.event_when_killed() {
                                state.control_flags.set_tick_world(true);
                                state.control_flags.set_interactions_disabled(true);
                                state.textscript_vm.start_script(npc.event_num);
                            } else {
                                npc.cond.set_explode_die(true);
                            }
                        } else {
                            if npc.shock < 14 {
                                if let Some(table_entry) = state.npc_table.get_entry(npc.npc_type) {
                                    state.sound_manager.play_sfx(table_entry.hurt_sound);
                                }

                                npc.shock = 16;

                                for _ in 0..3 {
                                    state.create_caret((bullet.x + npc.x) / 2, (bullet.y + npc.y) / 2, CaretType::HurtParticles, Direction::Left);
                                }
                            }

                            if npc.npc_flags.show_damage() {
                                // todo show damage
                            }
                        }
                    } else if !bullet.weapon_flags.hit_right_slope()
                        && bullet.btype != 13 && bullet.btype != 14 && bullet.btype != 15
                        && bullet.btype != 28 && bullet.btype != 29 && bullet.btype != 30 {
                        state.create_caret((bullet.x + npc.x) / 2, (bullet.y + npc.y) / 2, CaretType::ProjectileDissipation, Direction::Right);
                        state.sound_manager.play_sfx(31);
                        bullet.life = 0;
                        continue;
                    }

                    if bullet.life > 0 {
                        bullet.life -= 1;
                    }
                }

                if npc.cond.explode_die() && !npc.cond.drs_destroyed() {
                    dead_npcs.push(npc.id);
                }

                npc.cond.set_drs_destroyed(false);
            }
        }

        for i in 0..self.npc_map.boss_map.parts.len() {
            let mut idx = i;
            let (mut destroy_x, mut destroy_y, mut destroy_radius, mut destroy_count) = (0, 0, 0, 0);
            let mut npc = unsafe { self.npc_map.boss_map.parts.get_unchecked_mut(i) };
            if !npc.cond.alive() {
                continue;
            }

            for bullet in self.bullet_manager.bullets.iter_mut() {
                if !bullet.cond.alive() || bullet.damage < 1 {
                    continue;
                }

                let hit = (
                    npc.npc_flags.shootable()
                        && (npc.x - npc.hit_bounds.right as isize) < (bullet.x + bullet.enemy_hit_width as isize)
                        && (npc.x + npc.hit_bounds.right as isize) > (bullet.x - bullet.enemy_hit_width as isize)
                        && (npc.y - npc.hit_bounds.top as isize) < (bullet.y + bullet.enemy_hit_height as isize)
                        && (npc.y + npc.hit_bounds.bottom as isize) > (bullet.y - bullet.enemy_hit_height as isize)
                ) || (
                    npc.npc_flags.invulnerable()
                        && (npc.x - npc.hit_bounds.right as isize) < (bullet.x + bullet.hit_bounds.right as isize)
                        && (npc.x + npc.hit_bounds.right as isize) > (bullet.x - bullet.hit_bounds.left as isize)
                        && (npc.y - npc.hit_bounds.top as isize) < (bullet.y + bullet.hit_bounds.bottom as isize)
                        && (npc.y + npc.hit_bounds.bottom as isize) > (bullet.y - bullet.hit_bounds.top as isize)
                );

                if !hit {
                    continue;
                }

                if npc.npc_flags.shootable() {
                    if npc.cond.damage_boss() {
                        idx = 0;
                        npc = unsafe { self.npc_map.boss_map.parts.get_unchecked_mut(0) };
                    }

                    npc.life = npc.life.saturating_sub(bullet.damage);

                    if npc.life == 0 {
                        npc.life = npc.id;

                        if self.player.cond.alive() && npc.npc_flags.event_when_killed() {
                            state.control_flags.set_tick_world(true);
                            state.control_flags.set_interactions_disabled(true);
                            state.textscript_vm.start_script(npc.event_num);
                        } else {
                            state.sound_manager.play_sfx(self.npc_map.boss_map.death_sound[idx]);

                            destroy_x = npc.x;
                            destroy_y = npc.y;
                            destroy_radius = npc.display_bounds.right;
                            destroy_count = 4usize * (2usize).pow((npc.size as u32).saturating_sub(1));

                            npc.cond.set_alive(false);
                        }
                    } else {
                        if npc.shock < 14 {
                            for _ in 0..3 {
                                state.create_caret(bullet.x, bullet.y, CaretType::HurtParticles, Direction::Left);
                            }
                            state.sound_manager.play_sfx(self.npc_map.boss_map.hurt_sound[idx]);
                        }

                        npc.shock = 8;

                        npc = unsafe { self.npc_map.boss_map.parts.get_unchecked_mut(0) };
                        npc.shock = 8;
                    }

                    bullet.life = bullet.life.saturating_sub(1);
                    if bullet.life < 1 {
                        bullet.cond.set_alive(false);
                    }
                } else if [13, 14, 15, 28, 29, 30].contains(&bullet.btype) {
                    bullet.life = bullet.life.saturating_sub(1);
                } else if !bullet.weapon_flags.hit_right_slope() {
                    state.create_caret(bullet.x, bullet.y, CaretType::ProjectileDissipation, Direction::Right);
                    state.sound_manager.play_sfx(31);
                    bullet.life = 0;
                    continue;
                }
            }

            if destroy_count != 0 {
                self.npc_map.create_death_effect(destroy_x, destroy_y, destroy_radius, destroy_count, state);
            }
        }

        if !dead_npcs.is_empty() {
            let missile = self.inventory.has_weapon(WeaponType::MissileLauncher)
                | self.inventory.has_weapon(WeaponType::SuperMissileLauncher);
            self.npc_map.process_dead_npcs(&dead_npcs, missile, state);
            self.npc_map.garbage_collect();
        }
    }

    fn tick_world(&mut self, state: &mut SharedGameState) -> GameResult {
        self.stage_select_text_y_pos = 54;
        self.current_teleport_slot = 0;

        self.player.current_weapon = {
            if let Some(weapon) = self.inventory.get_current_weapon_mut() {
                weapon.wtype as u8
            } else {
                0
            }
        };
        self.player.tick(state, ())?;

        if self.player.damage > 0 {
            let xp_loss = self.player.damage * if self.player.equip.has_arms_barrier() { 1 } else { 2 };
            match self.inventory.take_xp(xp_loss, state) {
                TakeExperienceResult::LevelDown if self.player.life > 0 => {
                    state.create_caret(self.player.x, self.player.y, CaretType::LevelUp, Direction::Right);
                }
                _ => {}
            }

            self.player.damage = 0;
        }

        for npc_id in self.npc_map.npc_ids.iter() {
            if let Some(npc_cell) = self.npc_map.npcs.get(npc_id) {
                let mut npc = npc_cell.borrow_mut();

                if npc.cond.alive() {
                    npc.tick(state, (&mut self.player, &self.npc_map.npcs, &mut self.stage))?;
                }
            }
        }
        self.npc_map.boss_map.tick(state, (&mut self.player, &self.npc_map.npcs, &mut self.stage))?;
        self.npc_map.process_npc_changes(state);
        self.npc_map.garbage_collect();

        self.player.tick_map_collisions(state, &mut self.stage);
        self.player.tick_npc_collisions(state, &mut self.npc_map, &mut self.inventory);

        for npc_id in self.npc_map.npc_ids.iter() {
            if let Some(npc_cell) = self.npc_map.npcs.get(npc_id) {
                let mut npc = npc_cell.borrow_mut();

                if npc.cond.alive() && !npc.npc_flags.ignore_solidity() {
                    npc.tick_map_collisions(state, &mut self.stage);
                }
            }
        }
        for npc in self.npc_map.boss_map.parts.iter_mut() {
            npc.tick_map_collisions(state, &mut self.stage);
        }
        self.npc_map.process_npc_changes(state);
        self.npc_map.garbage_collect();

        self.tick_npc_bullet_collissions(state);
        self.npc_map.process_npc_changes(state);

        self.bullet_manager.tick_bullets(state, &self.player, &mut self.stage);
        state.tick_carets();

        match self.frame.update_target {
            UpdateTarget::Player => {
                self.frame.target_x = self.player.target_x;
                self.frame.target_y = self.player.target_y;
            }
            UpdateTarget::NPC(npc_id) => {
                if let Some(npc_cell) = self.npc_map.npcs.get(&npc_id) {
                    let mut npc = npc_cell.borrow();

                    if npc.cond.alive() {
                        self.frame.target_x = npc.x;
                        self.frame.target_y = npc.y;
                    }
                }
            }
            UpdateTarget::Boss(boss_id) => {
                if let Some(boss) = self.npc_map.boss_map.parts.get(boss_id as usize) {
                    if boss.cond.alive() {
                        self.frame.target_x = boss.x;
                        self.frame.target_y = boss.y;
                    }
                }
            }
        }
        self.frame.update(state, &self.stage);

        if state.control_flags.control_enabled() {
            if let Some(weapon) = self.inventory.get_current_weapon_mut() {
                weapon.shoot_bullet(&self.player, &mut self.bullet_manager, state);
            }

            if state.key_trigger.weapon_next() {
                state.sound_manager.play_sfx(4);
                self.inventory.next_weapon();
                self.weapon_x_pos = 32;
            }

            if state.key_trigger.weapon_prev() {
                state.sound_manager.play_sfx(4);
                self.inventory.prev_weapon();
                self.weapon_x_pos = 0;
            }

            // update health bar
            if self.life_bar < self.player.life as u16 {
                self.life_bar = self.player.life as u16;
            }

            if self.life_bar > self.player.life as u16 {
                self.life_bar_counter += 1;
                if self.life_bar_counter > 30 {
                    self.life_bar -= 1;
                }
            } else {
                self.life_bar_counter = 0;
            }

            self.boss_life_bar.tick(state, &self.npc_map)?;
        }

        Ok(())
    }

    fn tick_stage_select(&mut self, state: &mut SharedGameState) -> GameResult {
        let slot_count = state.teleporter_slots.iter()
            .filter(|&&(index, _event_num)| index != 0)
            .count();

        if self.stage_select_text_y_pos > 46 {
            self.stage_select_text_y_pos -= 1;
        }

        if state.key_trigger.left() {
            if self.current_teleport_slot == 0 {
                self.current_teleport_slot = slot_count.saturating_sub(1) as u8;
            } else {
                self.current_teleport_slot -= 1;
            }
        } else if state.key_trigger.right() {
            if self.current_teleport_slot == slot_count.saturating_sub(1) as u8 {
                self.current_teleport_slot = 0;
            } else {
                self.current_teleport_slot += 1;
            }
        }

        if state.key_trigger.left() || state.key_trigger.right() {
            state.sound_manager.play_sfx(1);
            if let Some(&(index, _event_num)) = state.teleporter_slots.get(self.current_teleport_slot as usize) {
                state.textscript_vm.start_script(1000 + index);
            } else {
                state.textscript_vm.start_script(1000);
            }
        }

        if state.key_trigger.jump() | state.key_trigger.fire() {
            state.textscript_vm.set_mode(ScriptMode::Map);
            state.control_flags.set_tick_world(true);
            state.control_flags.set_control_enabled(true);
            state.control_flags.set_interactions_disabled(false);

            if state.key_trigger.jump() {
                if let Some(&(_index, event_num)) = state.teleporter_slots.get(self.current_teleport_slot as usize) {
                    state.textscript_vm.start_script(event_num);
                }
            }
        }

        Ok(())
    }

    fn draw_stage_select(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "StageImage")?;

        let slot_count = state.teleporter_slots.iter()
            .filter(|&&(index, _event_num)| index != 0)
            .count();
        let slot_offset = ((state.canvas_size.0 - 40.0 * slot_count as f32) / 2.0).floor();
        let mut slot_rect = Rect::new(0, 0, 0, 0);

        for i in 0..slot_count {
            let index = state.teleporter_slots[i].0;

            slot_rect.left = 32 * (index as usize % 8);
            slot_rect.top = 16 * (index as usize / 8);
            slot_rect.right = slot_rect.left + 32;
            slot_rect.bottom = slot_rect.top + 16;

            batch.add_rect(slot_offset + i as f32 * 40.0, 64.0, &slot_rect);
        }

        batch.draw(ctx)?;

        let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "TextBox")?;

        batch.add_rect(128.0, self.stage_select_text_y_pos as f32, &state.constants.textscript.stage_select_text);
        if slot_count > 0 {
            batch.add_rect(slot_offset + self.current_teleport_slot as f32 * 40.0, 64.0, &state.constants.textscript.cursor[self.tick / 2 % 2]);
        }

        batch.draw(ctx)?;

        Ok(())
    }

    fn draw_debug_outlines(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        for npc in self.npc_map.npcs.values() {
            let npc = npc.borrow();

            if npc.x < (self.frame.x - 128 - npc.display_bounds.width() as isize * 0x200)
                || npc.x > (self.frame.x + 128 + (state.canvas_size.0 as isize + npc.display_bounds.width() as isize) * 0x200)
                && npc.y < (self.frame.y - 128 - npc.display_bounds.height() as isize * 0x200)
                || npc.y > (self.frame.y + 128 + (state.canvas_size.1 as isize + npc.display_bounds.height() as isize) * 0x200) {
                continue;
            }

            // todo faster way to draw dynamic rectangles
            // // top
            // state.texture_set.draw_rect(Rect::new_size((npc.x - npc.hit_bounds.right as isize - self.frame.x) / 0x200,
            //                                            (npc.y - npc.hit_bounds.top as isize - self.frame.y) / 0x200,
            //                                            (npc.hit_bounds.right + npc.hit_bounds.right) as isize / 0x200,
            //                                            1),
            //                             [0.0, if npc.flags.hit_top_wall() { 1.0 } else { 0.0 }, 1.0, 1.0], ctx)?;
            // // bottom
            // state.texture_set.draw_rect(Rect::new_size((npc.x - npc.hit_bounds.right as isize - self.frame.x) / 0x200,
            //                                            (npc.y + npc.hit_bounds.bottom as isize - self.frame.y) / 0x200 - 1,
            //                                            (npc.hit_bounds.right + npc.hit_bounds.right) as isize / 0x200,
            //                                            1),
            //                             [0.0, if npc.flags.hit_bottom_wall() { 1.0 } else { 0.0 }, 1.0, 1.0], ctx)?;
            // // left
            // state.texture_set.draw_rect(Rect::new_size((npc.x - npc.hit_bounds.right as isize - self.frame.x) / 0x200,
            //                                            (npc.y - npc.hit_bounds.top as isize - self.frame.y) / 0x200,
            //                                            1,
            //                                            (npc.hit_bounds.top + npc.hit_bounds.bottom) as isize / 0x200),
            //                             [0.0, if npc.flags.hit_left_wall() { 1.0 } else { 0.0 }, 1.0, 1.0], ctx)?;
            // // right
            // state.texture_set.draw_rect(Rect::new_size((npc.x + npc.hit_bounds.right as isize - self.frame.x) / 0x200 - 1,
            //                                            (npc.y - npc.hit_bounds.top as isize - self.frame.y) / 0x200,
            //                                            1,
            //                                            (npc.hit_bounds.top + npc.hit_bounds.bottom) as isize / 0x200),
            //                             [0.0, if npc.flags.hit_right_wall() { 1.0 } else { 0.0 }, 1.0, 1.0], ctx)?;

            {
                let hit_rect_size = clamp(npc.hit_rect_size(), 1, 4);
                let hit_rect_size = hit_rect_size * hit_rect_size;

                let x = (npc.x + npc.offset_x()) / (16 * 0x200);
                let y = (npc.y + npc.offset_y()) / (16 * 0x200);
                let batch = state.texture_set.get_or_load_batch(ctx, &state.constants, "Caret")?;

                let caret_rect = Rect::new_size(2, 74, 4, 4);
                let caret2_rect = Rect::new_size(65, 9, 6, 6);

                for (idx, (&ox, &oy)) in crate::physics::OFF_X.iter()
                    .zip(crate::physics::OFF_Y.iter()).enumerate() {
                    if idx == hit_rect_size {
                        break;
                    }

                    batch.add_rect(((x + ox) * 16 - self.frame.x / 0x200) as f32 - 2.0,
                                   ((y + oy) * 16 - self.frame.y / 0x200) as f32 - 2.0,
                                   &caret_rect);
                }


                batch.add_rect(((npc.x - self.frame.x) / 0x200) as f32 - 3.0,
                               ((npc.y - self.frame.y) / 0x200) as f32 - 3.0,
                               &caret2_rect);

                batch.draw(ctx)?;
            }

            let text = format!("{}:{}:{}", npc.id, npc.npc_type, npc.size);
            state.font.draw_colored_text(text.chars(), ((npc.x - self.frame.x) / 0x200) as f32, ((npc.y - self.frame.y) / 0x200) as f32,
                                         (0, 255, (npc.id & 0xff) as u8), &state.constants, &mut state.texture_set, ctx)?;
        }
        Ok(())
    }
}

impl Scene for GameScene {
    fn init(&mut self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        let seed = (self.player.max_life as i32)
            .wrapping_add(self.player.x as i32)
            .wrapping_add(self.player.y as i32)
            .wrapping_add(self.stage_id as i32)
            .wrapping_mul(7);
        state.game_rng = RNG::new(seed);
        state.textscript_vm.set_scene_script(self.stage.load_text_script(&state.base_path, &state.constants, ctx)?);
        state.textscript_vm.suspend = false;

        let npcs = self.stage.load_npcs(&state.base_path, ctx)?;
        for npc_data in npcs.iter() {
            log::info!("creating npc: {:?}", npc_data);

            let npc = self.npc_map.create_npc_from_data(&state.npc_table, npc_data);
            if npc.npc_flags.appear_when_flag_set() {
                if let Some(true) = state.game_flags.get(npc_data.flag_num as usize) {
                    npc.cond.set_alive(true);
                }
            } else if npc.npc_flags.hide_unless_flag_set() {
                if let Some(false) = state.game_flags.get(npc_data.flag_num as usize) {
                    npc.cond.set_alive(true);
                }
            } else {
                npc.cond.set_alive(true);
            }
        }

        state.npc_table.tileset_name = self.tex_tileset_name.to_owned();
        state.npc_table.tex_npc1_name = ["Npc/", &self.stage.data.npc1.filename()].join("");
        state.npc_table.tex_npc2_name = ["Npc/", &self.stage.data.npc2.filename()].join("");

        if state.constants.is_cs_plus {
            match state.season {
                Season::Halloween => self.player.appearance = PlayerAppearance::HalloweenQuote,
                Season::Christmas => self.player.appearance = PlayerAppearance::ReindeerQuote,
                _ => {}
            }
        }

        self.npc_map.boss_map.boss_type = self.stage.data.boss_no as u16;
        self.frame.target_x = self.player.x;
        self.frame.target_y = self.player.y;
        self.frame.immediate_update(state, &self.stage);

        Ok(())
    }

    fn draw_tick(&mut self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        self.frame.prev_x = self.frame.x;
        self.frame.prev_y = self.frame.y;
        self.player.prev_x = self.player.x;
        self.player.prev_y = self.player.y;

        for npc_cell in self.npc_map.npcs.values() {
            let mut npc = npc_cell.borrow_mut();

            if npc.cond.alive() {
                npc.prev_x = npc.x;
                npc.prev_y = npc.y;
            }
        }

        for npc in self.npc_map.boss_map.parts.iter_mut() {
            if npc.cond.alive() {
                npc.prev_x = npc.x;
                npc.prev_y = npc.y;
            }
        }

        Ok(())
    }

    fn tick(&mut self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        state.update_key_trigger();

        if self.intro_mode && (state.key_trigger.jump() || self.tick >= 500) {
            state.next_scene = Some(Box::new(TitleScene::new()));
        }

        match state.textscript_vm.mode {
            ScriptMode::Map if state.control_flags.tick_world() => self.tick_world(state)?,
            ScriptMode::StageSelect => self.tick_stage_select(state)?,
            _ => {}
        }

        if self.map_name_counter > 0 {
            self.map_name_counter -= 1;
        }

        if self.weapon_x_pos > 16 {
            self.weapon_x_pos -= 2;
        } else if self.weapon_x_pos < 16 {
            self.weapon_x_pos += 2;
        }

        match state.fade_state {
            FadeState::FadeOut(tick, direction) if tick < 15 => {
                state.fade_state = FadeState::FadeOut(tick + 1, direction);
            }
            FadeState::FadeOut(tick, _) if tick == 15 => {
                state.fade_state = FadeState::Hidden;
            }
            FadeState::FadeIn(tick, direction) if tick > -15 => {
                state.fade_state = FadeState::FadeIn(tick - 1, direction);
            }
            FadeState::FadeIn(tick, _) if tick == -15 => {
                state.fade_state = FadeState::Visible;
            }
            _ => {}
        }

        TextScriptVM::run(state, self, ctx)?;
        self.tick = self.tick.wrapping_add(1);
        Ok(())
    }

    fn draw(&self, state: &mut SharedGameState, ctx: &mut Context) -> GameResult {
        self.draw_background(state, ctx)?;
        self.draw_tiles(state, ctx, TileLayer::Background)?;
        if state.settings.lighting_efects
            && self.stage.data.background_type != BackgroundType::Black
            && self.stage.data.background_type != BackgroundType::Outside
            && self.stage.data.background_type != BackgroundType::OutsideWind {
            self.draw_light_map(state, ctx)?;
        }

        for npc_id in self.npc_map.npc_ids.iter() {
            if let Some(npc_cell) = self.npc_map.npcs.get(npc_id) {
                let npc = npc_cell.borrow();

                if npc.x < (self.frame.x - 128 - npc.display_bounds.width() as isize * 0x200)
                    || npc.x > (self.frame.x + 128 + (state.canvas_size.0 as isize + npc.display_bounds.width() as isize) * 0x200)
                    && npc.y < (self.frame.y - 128 - npc.display_bounds.height() as isize * 0x200)
                    || npc.y > (self.frame.y + 128 + (state.canvas_size.1 as isize + npc.display_bounds.height() as isize) * 0x200) {
                    continue;
                }

                npc.draw(state, ctx, &self.frame)?;
            }
        }
        self.npc_map.boss_map.draw(state, ctx, &self.frame)?;
        self.draw_bullets(state, ctx)?;
        self.player.draw(state, ctx, &self.frame)?;
        self.draw_tiles(state, ctx, TileLayer::Foreground)?;
        self.draw_tiles(state, ctx, TileLayer::Snack)?;
        self.draw_carets(state, ctx)?;
        if state.settings.lighting_efects
            && self.stage.data.background_type == BackgroundType::Black {
            self.draw_light_map(state, ctx)?;
        }

        self.draw_black_bars(state, ctx)?;

        if state.control_flags.control_enabled() {
            self.draw_hud(state, ctx)?;
            self.boss_life_bar.draw(state, ctx, &self.frame)?;
        }

        if state.textscript_vm.mode == ScriptMode::StageSelect {
            self.draw_stage_select(state, ctx)?;
        }

        self.draw_fade(state, ctx)?;
        if self.map_name_counter > 0 {
            let map_name = if self.intro_mode {
                state.constants.title.intro_text.chars()
            } else {
                self.stage.data.name.chars()
            };
            let width = state.font.text_width(map_name.clone(), &state.constants);

            state.font.draw_text(map_name,
                                 ((state.canvas_size.0 - width) / 2.0).floor(), 80.0,
                                 &state.constants, &mut state.texture_set, ctx)?;
        }

        self.draw_text_boxes(state, ctx)?;

        if state.settings.debug_outlines {
            self.draw_debug_outlines(state, ctx)?;
        }

        self.draw_number(state.canvas_size.0 - 8.0, 8.0, timer::fps(ctx) as usize, Alignment::Right, state, ctx)?;
        Ok(())
    }

    fn debug_overlay_draw(&mut self, components: &mut Components, state: &mut SharedGameState, ctx: &mut Context, ui: &mut imgui::Ui) -> GameResult {
        components.live_debugger.run_ingame(self, state, ctx, ui)?;
        Ok(())
    }
}
