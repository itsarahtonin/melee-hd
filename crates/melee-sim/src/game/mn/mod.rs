//! The menus (`melee/mn`): the main menu and its lists (`mnmain.c`), the Data menu's movies
//! (`mngallery.c`), the menus' shared helpers (`mn_22EC.c`), the character and stage selects
//! (`mncharsel.c`, `mnstagesel.c`), the VS menu's rules and names (`mnmainrule.c`,
//! `mnruleplus.c`, `mnitemsw.c`, `mnstagesw.c`, `mnname.c`, `mnnamenew.c`), and the Options
//! menus (`mnvibration.c`, `mnsound.c`, `mndeflicker.c`, `mnlanguage.c`, `mndatadel.c`), and the Data
//! menu's screens (`mninfo.c` and the others `data` lists).

pub mod charsel;
pub mod common;
pub mod count;
pub mod data;
pub mod datadel;
pub mod deflicker;
pub mod diagram;
pub mod gallery;
pub mod info;
pub mod info_bonus;
pub mod item_switch;
pub mod language;
pub mod main;
pub mod names;
pub mod options;
pub mod rule_plus;
pub mod rules;
pub mod sound;
pub mod stage_switch;
pub mod stagesel;
pub mod vibration;
