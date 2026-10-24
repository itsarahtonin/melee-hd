//! The menus (`melee/mn`): the main menu and its lists (`mnmain.c`), the Data menu's movies
//! (`mngallery.c`), the menus' shared helpers (`mn_22EC.c`), the character and stage selects
//! (`mncharsel.c`, `mnstagesel.c`), and the VS menu's rules and names (`mnmainrule.c`,
//! `mnruleplus.c`, `mnitemsw.c`, `mnstagesw.c`, `mnname.c`, `mnnamenew.c`).

pub mod charsel;
pub mod common;
pub mod gallery;
pub mod item_switch;
pub mod main;
pub mod names;
pub mod rule_plus;
pub mod rules;
pub mod stage_switch;
pub mod stagesel;
