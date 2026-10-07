//! Melee's game data, read from the player's own disc at run time: the disc's file system
//! (`disc`) and HAL's DAT archives (`archive`). Nothing from the game is embedded.

pub mod archive;
pub mod disc;
