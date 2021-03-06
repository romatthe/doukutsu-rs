use num_traits::abs;
use num_traits::clamp;

use crate::common::Direction;
use crate::ggez::GameResult;
use crate::npc::NPC;
use crate::player::Player;
use crate::shared_game_state::SharedGameState;

impl NPC {
    pub(crate) fn tick_n024_power_critter(&mut self, state: &mut SharedGameState, player: &Player) -> GameResult {
        match self.action_num {
            0 | 1 => {
                if self.action_num == 0 {
                    self.y += 3 * 0x200;
                    self.action_num = 1;
                }

                if self.action_counter >= 8
                    && abs(self.x - player.x) < (112 * 0x200)
                    && abs(self.y - player.y) < (80 * 0x200) {
                    if self.x > player.x {
                        self.direction = Direction::Left;
                    } else {
                        self.direction = Direction::Right;
                    }

                    self.anim_num = 1;
                } else {
                    if self.action_counter < 8 {
                        self.action_counter += 1;
                    }

                    self.anim_num = 0;
                }

                if self.shock > 0 {
                    self.action_num = 2;
                    self.action_counter = 0;
                    self.anim_num = 0;
                }

                if self.action_counter >= 8
                    && abs(self.x - player.x) < 96 * 0x200
                    && self.y - 96 * 0x200 < player.y
                    && self.y + 48 * 0x200 > player.y {
                    self.action_num = 2;
                    self.action_counter = 0;
                    self.anim_num = 0;
                }
            }
            2 => {
                self.action_counter += 1;
                if self.action_counter > 8 {
                    if self.x > player.x {
                        self.direction = Direction::Left;
                    } else {
                        self.direction = Direction::Right;
                    }

                    self.action_num = 3;
                    self.anim_num = 2;
                    self.vel_x = self.direction.vector_x() * 0x100;
                    self.vel_y = -0x5ff;
                    state.sound_manager.play_sfx(108);
                }
            }
            3 => {
                if self.vel_y > 0x200 {
                    self.target_y = self.y;
                    self.action_num = 4;
                    self.action_counter = 0;
                    self.anim_num = 3;
                }
            }
            4 => {
                if self.x > player.x {
                    self.direction = Direction::Left;
                } else {
                    self.direction = Direction::Right;
                }

                self.action_counter += 1;

                if (self.flags.hit_left_wall()
                    || self.flags.hit_right_wall()
                    || self.flags.hit_top_wall()) || self.action_counter > 100 {
                    self.action_num = 5;
                    self.anim_num = 2;
                    self.vel_x /= 2;
                    self.damage = 12;
                }

                if self.action_counter % 4 == 1 {
                    state.sound_manager.play_sfx(110);
                }

                self.anim_counter += 1;
                if self.anim_counter > 0 {
                    self.anim_counter = 0;
                    self.anim_num += 1;
                    if self.anim_num > 5 {
                        self.anim_num = 3;
                    }
                }
            }
            5 => {
                if self.flags.hit_bottom_wall() {
                    self.vel_x = 0;
                    self.action_num = 1;
                    self.action_counter = 0;
                    self.anim_num = 0;
                    self.damage = 2;

                    state.sound_manager.play_sfx(23);
                    state.quake_counter = 30;
                }
            }
            _ => {}
        }

        if self.action_num != 4 {
            self.vel_y += 0x40;
            if self.vel_y > 0x5ff {
                self.vel_y = 0x5ff;
            }
        } else {
            self.vel_x = clamp(self.vel_x + if self.x < player.x { 0x20 } else { -0x20 }, -0x200, 0x200);
            self.vel_y = clamp(self.vel_x + if self.y > self.target_y { -0x10 } else { 0x10 }, -0x200, 0x200);
        }

        self.x += self.vel_x;
        self.y += self.vel_y;

        let dir_offset = if self.direction == Direction::Left { 0 } else { 6 };
        self.anim_rect = state.constants.npc.n024_power_critter[self.anim_num as usize + dir_offset];

        Ok(())
    }

    pub(crate) fn tick_n026_bat_flying(&mut self, state: &mut SharedGameState, player: &Player) -> GameResult {
        match self.action_num {
            0 | 1 => {
                if self.action_num == 0 {
                    let angle = state.game_rng.range(0..0xff);
                    self.vel_x = ((angle as f64 * 1.40625).cos() * 512.0) as isize;
                    self.target_x = self.x + ((angle as f64 * 1.40625 + std::f64::consts::FRAC_2_PI).cos() * 8.0 * 512.0) as isize;

                    let angle = state.game_rng.range(0..0xff);
                    self.vel_y = ((angle as f64 * 1.40625).sin() * 512.0) as isize;
                    self.target_y = self.y + ((angle as f64 * 1.40625 + std::f64::consts::FRAC_2_PI).sin() * 8.0 * 512.0) as isize;

                    self.action_num = 1;
                    self.action_counter2 = 120;
                }

                if self.x > player.x {
                    self.direction = Direction::Left;
                } else {
                    self.direction = Direction::Right;
                }

                self.vel_x = clamp(self.vel_x + 0x10 * (self.target_x - self.x).signum(), -0x200, 0x200);
                self.vel_y = clamp(self.vel_y + 0x10 * (self.target_y - self.y).signum(), -0x200, 0x200);

                if self.action_counter2 < 120 {
                    self.action_counter2 += 1;
                } else if abs(self.x - player.x) < 8 * 0x200
                    && self.y < player.y
                    && self.y + 96 * 0x200 > player.y {
                    self.vel_x /= 2;
                    self.vel_y = 0;
                    self.action_num = 3;
                    self.npc_flags.set_ignore_solidity(false);
                }
            }
            3 => {
                self.vel_y += 0x40;
                if self.vel_y > 0x5ff {
                    self.vel_y = 0x5ff;
                }

                if self.flags.hit_bottom_wall() {
                    self.vel_x *= 2;
                    self.vel_y = 0;
                    self.action_counter2 = 0;
                    self.action_num = 1;
                    self.npc_flags.set_ignore_solidity(true);
                }
            }
            _ => {}
        }

        self.x += self.vel_x;
        self.y += self.vel_y;

        if self.action_num == 3 {
            self.anim_num = 3;
        } else {
            self.anim_counter += 1;
            if self.anim_counter > 1 {
                self.anim_counter = 0;
                self.anim_num += 1;
                if self.anim_num > 2 {
                    self.anim_num = 0;
                }
            }
        }

        let dir_offset = if self.direction == Direction::Left { 0 } else { 4 };
        self.anim_rect = state.constants.npc.n026_bat_flying[self.anim_num as usize + dir_offset];

        Ok(())
    }

    pub(crate) fn tick_n028_flying_critter(&mut self, state: &mut SharedGameState, player: &Player) -> GameResult {
        match self.action_num {
            0 | 1 => {
                if self.action_num == 0 {
                    self.y += 3 * 0x200;
                    self.action_num = 1;
                }

                if self.action_counter >= 8
                    && abs(self.x - player.x) < 96 * 0x200
                    && self.y - 128 * 0x200 < player.y
                    && self.y + 48 * 0x200 > player.y {
                    if self.x > player.x {
                        self.direction = Direction::Left;
                    } else {
                        self.direction = Direction::Right;
                    }

                    self.anim_num = 1;
                } else {
                    if self.action_counter < 8 {
                        self.action_counter += 1;
                    }

                    self.anim_num = 0;
                }

                if self.shock > 0 {
                    self.action_num = 2;
                    self.action_counter = 0;
                    self.anim_num = 0;
                }

                if self.action_counter >= 8
                    && abs(self.x - player.x) < 96 * 0x200
                    && self.y - 96 * 0x200 < player.y
                    && self.y + 48 * 0x200 > player.y {
                    self.action_num = 2;
                    self.action_counter = 0;
                    self.anim_num = 0;
                }
            }
            2 => {
                self.action_counter += 1;
                if self.action_counter > 8 {
                    if self.x > player.x {
                        self.direction = Direction::Left;
                    } else {
                        self.direction = Direction::Right;
                    }

                    self.action_num = 3;
                    self.anim_num = 2;
                    self.vel_x = self.direction.vector_x() * 0x100;
                    self.vel_y = -0x4cc;
                    state.sound_manager.play_sfx(30);
                }
            }
            3 => {
                if self.vel_y > 0x100 {
                    self.target_y = self.y;
                    self.action_num = 4;
                    self.action_counter = 0;
                    self.anim_num = 3;
                }
            }
            4 => {
                if self.x > player.x {
                    self.direction = Direction::Left;
                } else {
                    self.direction = Direction::Right;
                }

                self.action_counter += 1;

                if (self.flags.hit_left_wall()
                    || self.flags.hit_right_wall()
                    || self.flags.hit_top_wall()) || self.action_counter > 100 {
                    self.action_num = 5;
                    self.anim_num = 2;
                    self.vel_x /= 2;
                    self.damage = 3;
                }

                if self.action_counter % 4 == 1 {
                    state.sound_manager.play_sfx(110);
                }

                if self.flags.hit_bottom_wall() {
                    self.vel_y = -0x200;
                }

                self.anim_counter += 1;
                if self.anim_counter > 0 {
                    self.anim_counter = 0;
                    self.anim_num += 1;
                    if self.anim_num > 5 {
                        self.anim_num = 3;
                    }
                }
            }
            5 => {
                if self.flags.hit_bottom_wall() {
                    self.vel_x = 0;
                    self.action_num = 1;
                    self.action_counter = 0;
                    self.anim_num = 0;
                    self.damage = 2;

                    state.sound_manager.play_sfx(23);
                }
            }
            _ => {}
        }

        if self.action_num != 4 {
            self.vel_y += 0x40;
            if self.vel_y > 0x5ff {
                self.vel_y = 0x5ff;
            }
        } else {
            self.vel_x = clamp(self.vel_x + if self.x < player.x { 0x20 } else { -0x20 }, -0x200, 0x200);
            self.vel_y = clamp(self.vel_x + if self.y > self.target_y { -0x10 } else { 0x10 }, -0x200, 0x200);
        }

        self.x += self.vel_x;
        self.y += self.vel_y;

        let dir_offset = if self.direction == Direction::Left { 0 } else { 6 };
        self.anim_rect = state.constants.npc.n028_flying_critter[self.anim_num as usize + dir_offset];

        Ok(())
    }
}
