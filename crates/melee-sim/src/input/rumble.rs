//! HAL's controller rumble (`sysdolphin/baselib/rumble.c`): per port, a list of rumble scripts
//! ordered by priority, interpreted once per poll into a motor state for each port.
//!
//! Scripts are 16-bit commands: an opcode in the top three bits and a frame count in the rest.
//! The original drives the motors with `PADControlMotor` when a port's state changes; here
//! [`Rumble::interpret`] returns those changes and the frontend drives the motors.

use melee_data::archive::{Archives, DataRef};

/// A port's motor state, as the original's status values number them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum Motor {
    /// Braked (`PAD_MOTOR_STOP_HARD`).
    #[default]
    StopHard = 0,
    /// Coasting to a stop (`PAD_MOTOR_STOP`).
    Stop = 1,
    /// Running (`PAD_MOTOR_RUMBLE`).
    Rumble = 2,
}

impl Motor {
    /// The command `PADControlMotor` takes for this state.
    pub fn command(self) -> u32 {
        match self {
            Motor::StopHard => 2,
            Motor::Stop => 0,
            Motor::Rumble => 1,
        }
    }
}

/// The game's static rumble scripts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Table {
    /// `mnVibration_804D4FF0`: the rumble menu's one-frame pulse when a port turns rumble on.
    Vibration,
}

impl Table {
    pub const fn commands(self) -> &'static [u16] {
        match self {
            Table::Vibration => &[0x2001, 0x0000],
        }
    }
}

/// Where a script's next command is: the original's `HSD_Rumble*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum Script {
    /// In loaded game data (`LbRb.dat`'s `lbRumbleData`).
    Data(DataRef),
    /// `index` commands into one of the game's static scripts.
    Static { table: Table, index: u32 },
}

impl Script {
    /// The start of a static script.
    pub const fn table(table: Table) -> Script {
        Script::Static { table, index: 0 }
    }

    fn read(self, data: &Archives) -> u16 {
        match self {
            Script::Data(r) => data.u16(r),
            Script::Static { table, index } => table.commands()[index as usize],
        }
    }

    fn next(self) -> Script {
        match self {
            Script::Data(r) => Script::Data(r.at(2)),
            Script::Static { table, index } => Script::Static {
                table,
                index: index + 1,
            },
        }
    }
}

/// A node's slot among the library's nodes, which plays the part of its address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct NodeId(pub u16);

/// `HSD_PadRumbleListData`: one script running on a port. A freed node keeps its last
/// contents until reused, as the original's do.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Node {
    pub next: Option<NodeId>,
    pub id: u32,
    pub paused: bool,
    /// Lower runs first, so a higher priority's state wins.
    pub pri: u8,
    pub status: Motor,
    pub loop_count: u16,
    /// Frames left in the current command.
    pub wait: u16,
    /// Frames left to run: -1 forever, -2 until the script ends.
    pub frame: i32,
    /// Where a loop restarts.
    pub stack: Option<Script>,
    pub listp: Option<Script>,
    pub headp: Option<Script>,
}

impl Node {
    /// `HSD_PadRumbleInterpret1`: runs one frame of the script, writing its state into
    /// `status`. Returns whether the script is done.
    #[doc(alias = "HSD_PadRumbleInterpret1")]
    fn step(&mut self, data: &Archives, status: &mut Motor) -> bool {
        if self.paused {
            return false;
        }
        while self.wait == 0 {
            // The original reads through a null pointer here: a loop end before any loop start.
            let at = self
                .listp
                .expect("rumble script jumped to a null loop start");
            let command = at.read(data);
            let (op, frame) = (command >> 13, command & 0x1FFF);
            match op {
                0 => {
                    if self.frame == -2 {
                        return true;
                    }
                    self.listp = self.headp;
                }
                1..=3 => {
                    self.status = match op {
                        1 => Motor::Rumble,
                        2 => Motor::Stop,
                        _ => Motor::StopHard,
                    };
                    self.wait = frame;
                    self.listp = Some(at.next());
                }
                4 => {
                    self.loop_count = frame;
                    self.listp = Some(at.next());
                    self.stack = self.listp;
                }
                5 => {
                    self.loop_count = self.loop_count.wrapping_sub(1);
                    self.listp = if self.loop_count != 0 {
                        self.stack
                    } else {
                        Some(at.next())
                    };
                }
                // The original spins here forever; no script in the game has these.
                _ => panic!("rumble command {command:#06x} hangs the original"),
            }
        }
        *status = self.status;
        self.wait -= 1;
        if self.frame != -1 && self.frame != -2 {
            self.frame = self.frame.wrapping_sub(1);
            if self.frame == 0 {
                return true;
            }
        }
        false
    }
}

/// `HSD_RumbleData`: one port's rumble.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Port {
    /// The state last sent to the motor.
    pub last_status: Motor,
    pub status: Motor,
    /// The state when no script sets one.
    pub direct_status: Motor,
    pub nb_list: u16,
    pub list: Option<NodeId>,
}

/// The rumble library: `PadLibData`'s `rumble_info`, the nodes it hands out
/// (`gmMain_8046B1F8` in Melee) and the ports (`HSD_Rumble_804C22E0`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Rumble {
    /// The most scripts one port may run.
    pub max_list: u16,
    /// Ports hard-stop and scripts don't advance (`unk2`).
    pub suspended: bool,
    /// The first free node.
    pub free: Option<NodeId>,
    pub nodes: Vec<Node>,
    pub ports: [Port; 4],
}

impl Rumble {
    /// `HSD_PadRumbleInit`: `nb_list` free nodes, linked in order, and idle ports. With no
    /// nodes the free list is empty; the original keeps whatever pointer it was given, but no
    /// add can take it, as every port's limit is then 0.
    #[doc(alias = "HSD_PadRumbleInit")]
    pub fn new(nb_list: u16) -> Rumble {
        let mut nodes = vec![Node::default(); usize::from(nb_list)];
        for (i, n) in nodes.iter_mut().enumerate() {
            n.next = (i + 1 < usize::from(nb_list)).then(|| NodeId(i as u16 + 1));
        }
        Rumble {
            max_list: nb_list,
            suspended: false,
            free: (nb_list != 0).then_some(NodeId(0)),
            nodes,
            ports: [Port::default(); 4],
        }
    }

    /// `HSD_PadRumbleOn`: the port coasts (`Motor::Stop`) when no script runs.
    #[doc(alias = "HSD_PadRumbleOn")]
    pub fn on(&mut self, port: u8) {
        self.ports[usize::from(port)].direct_status = Motor::Stop;
    }

    /// `HSD_PadRumbleOffN`: the port brakes (`Motor::StopHard`) when no script runs.
    #[doc(alias = "HSD_PadRumbleOffN")]
    pub fn off(&mut self, port: u8) {
        self.ports[usize::from(port)].direct_status = Motor::StopHard;
    }

    /// `HSD_PadRumbleFree`: unlinks `node` from `port`'s scripts and frees it.
    #[doc(alias = "HSD_PadRumbleFree")]
    fn free(&mut self, port: usize, node: NodeId) {
        let p = &mut self.ports[port];
        let mut at = p.list.expect("the node is on the port's list");
        if at == node {
            p.list = self.nodes[usize::from(node.0)].next;
        } else {
            while self.nodes[usize::from(at.0)].next != Some(node) {
                at = self.nodes[usize::from(at.0)]
                    .next
                    .expect("the node is on the list");
            }
            self.nodes[usize::from(at.0)].next = self.nodes[usize::from(node.0)].next;
        }
        p.nb_list = p.nb_list.wrapping_sub(1);
        self.nodes[usize::from(node.0)].next = self.free;
        self.free = Some(node);
    }

    /// Frees every node on `port` that `drop` picks, in list order.
    fn free_where(&mut self, port: u8, mut drop: impl FnMut(&Node) -> bool) {
        let port = usize::from(port);
        let mut at = self.ports[port].list;
        while let Some(n) = at {
            at = self.nodes[usize::from(n.0)].next;
            if drop(&self.nodes[usize::from(n.0)]) {
                self.free(port, n);
            }
        }
    }

    /// `HSD_PadRumbleRemove`: stops every script on `port`.
    #[doc(alias = "HSD_PadRumbleRemove")]
    pub fn remove(&mut self, port: u8) {
        self.free_where(port, |_| true);
    }

    /// `HSD_PadRumbleRemoveAll`.
    #[doc(alias = "HSD_PadRumbleRemoveAll")]
    pub fn remove_all(&mut self) {
        for port in 0..4 {
            self.remove(port);
        }
    }

    /// `HSD_PadRumbleRemoveId`: stops `port`'s scripts added with `id`.
    #[doc(alias = "HSD_PadRumbleRemoveId")]
    pub fn remove_id(&mut self, port: u8, id: u32) {
        self.free_where(port, |n| n.id == id);
    }

    /// `HSD_PadRumblePause` (inlined in Melee): holds or resumes `port`'s scripts.
    pub fn pause(&mut self, port: u8, paused: bool) {
        let mut at = self.ports[usize::from(port)].list;
        while let Some(n) = at {
            let node = &mut self.nodes[usize::from(n.0)];
            at = node.next;
            node.paused = paused;
        }
    }

    /// `HSD_PadRumblePauseAll`.
    #[doc(alias = "HSD_PadRumblePauseAll")]
    pub fn pause_all(&mut self) {
        for port in 0..4 {
            self.pause(port, true);
        }
    }

    /// `HSD_PadRumbleUnpauseAll`.
    #[doc(alias = "HSD_PadRumbleUnpauseAll")]
    pub fn unpause_all(&mut self) {
        for port in 0..4 {
            self.pause(port, false);
        }
    }

    /// `HSD_PadRumbleAdd`: starts `script` on `port` for `frame` frames (-1 forever, -2 until
    /// it ends), after the scripts of priority `pri` or lower. Fails, returning false, when
    /// no node is free or the port runs `max_list` scripts.
    #[doc(alias = "HSD_PadRumbleAdd")]
    pub fn add(&mut self, port: u8, id: u32, frame: i32, pri: u8, script: Script) -> bool {
        let p = usize::from(port);
        let Some(n) = self.free else {
            return false;
        };
        // Never true while a node is free: the limit is the number of nodes.
        if self.ports[p].nb_list >= self.max_list {
            return false;
        }
        let node = &mut self.nodes[usize::from(n.0)];
        self.free = node.next;
        *node = Node {
            next: node.next,
            id,
            paused: false,
            pri,
            status: Motor::StopHard,
            loop_count: 0,
            wait: 0,
            frame,
            stack: None,
            listp: Some(script),
            headp: Some(script),
        };
        // `func_80378430_inline`: after every node of the same or lower priority.
        let mut prev: Option<NodeId> = None;
        let mut at = self.ports[p].list;
        while let Some(a) = at
            && self.nodes[usize::from(a.0)].pri <= pri
        {
            prev = at;
            at = self.nodes[usize::from(a.0)].next;
        }
        self.nodes[usize::from(n.0)].next = at;
        match prev {
            Some(prev) => self.nodes[usize::from(prev.0)].next = Some(n),
            None => self.ports[p].list = Some(n),
        }
        self.ports[p].nb_list = self.ports[p].nb_list.wrapping_add(1);
        true
    }

    /// `HSD_Rumble_80378524`: suspends or resumes all rumble.
    #[doc(alias = "HSD_Rumble_80378524")]
    pub fn suspend(&mut self, suspended: bool) {
        self.suspended = suspended;
    }

    /// `HSD_PadRumbleInterpret`: runs a frame of every port's scripts, in priority order, and
    /// returns the new state of each port whose state changed, which the original sends with
    /// `PADControlMotor`. A script that ends is freed.
    #[doc(alias = "HSD_PadRumbleInterpret")]
    pub fn interpret(&mut self, data: &Archives) -> [Option<Motor>; 4] {
        let mut changes = [None; 4];
        for (i, change) in changes.iter_mut().enumerate() {
            self.ports[i].status = Motor::StopHard;
            if !self.suspended {
                self.ports[i].status = self.ports[i].direct_status;
                let mut at = self.ports[i].list;
                while let Some(n) = at {
                    let node = &mut self.nodes[usize::from(n.0)];
                    at = node.next;
                    if node.step(data, &mut self.ports[i].status) {
                        self.free(i, n);
                    }
                }
            }
            let p = &mut self.ports[i];
            if p.status != p.last_status {
                *change = Some(p.status);
                p.last_status = p.status;
            }
        }
        changes
    }
}
