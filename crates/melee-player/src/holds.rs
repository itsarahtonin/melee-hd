//! `--holds SCRIPT`: scripted controllers for play-testing the menus without anyone at them,
//! in ssbm-rs's `MONKEY_HOLD` form: `HOLD@FROM-TO[,...]`, HOLD a `+`-joined list of buttons (a
//! b x y z l r start up down left right, or none), `sx=N`, `sy=N`, `cx=N` and `cy=N` for the
//! sticks and `p2` to `p4` for a port other than the first, held over ticks FROM to TO. Every
//! port has a controller in, at rest unless a hold names it.

use melee_sim::input::controller::PadData;
use melee_sim::input::pad::*;

#[derive(Clone, Copy, Debug)]
struct Hold {
    port: usize,
    button: u16,
    sticks: [i8; 4],
    from: u64,
    to: u64,
}

/// A parsed script.
#[derive(Clone, Debug, Default)]
pub struct Holds(Vec<Hold>);

impl Holds {
    pub fn parse(v: &str) -> Result<Holds, String> {
        v.split(',')
            .filter(|h| !h.trim().is_empty())
            .map(|h| parse_hold(h.trim()))
            .collect::<Result<_, _>>()
            .map(Holds)
    }

    /// The controllers at tick `t`.
    pub fn at(&self, t: u64) -> PadData {
        let mut pads = [PadStatus::default(); 4];
        for h in self.0.iter().filter(|h| (h.from..=h.to).contains(&t)) {
            let p = &mut pads[h.port];
            p.button |= h.button;
            [p.stick_x, p.stick_y, p.substick_x, p.substick_y] = h.sticks;
            if h.button & TRIGGER_L != 0 {
                p.trigger_left = 255;
            }
            if h.button & TRIGGER_R != 0 {
                p.trigger_right = 255;
            }
        }
        pads
    }
}

fn parse_hold(v: &str) -> Result<Hold, String> {
    let bad = || format!("--holds: {v} isn't HOLD@FROM-TO");
    let (names, range) = v.split_once('@').ok_or_else(bad)?;
    let (from, to) = range.split_once('-').ok_or_else(bad)?;
    let (mut button, mut sticks, mut port) = (0, [0i8; 4], 0);
    for name in names.split('+') {
        if let Some((stick, n)) = name.split_once('=') {
            let i = ["sx", "sy", "cx", "cy"]
                .iter()
                .position(|s| *s == stick)
                .ok_or_else(|| format!("--holds: no stick {stick}"))?;
            sticks[i] = n
                .parse()
                .map_err(|_| format!("--holds: {n} isn't -128 to 127"))?;
            continue;
        }
        if let Some(p) = name.strip_prefix('p').and_then(|p| p.parse::<usize>().ok()) {
            if !(1..=4).contains(&p) {
                return Err(format!("--holds: no port {p}"));
            }
            port = p - 1;
            continue;
        }
        button |= match name {
            "none" => 0,
            "a" => BUTTON_A,
            "b" => BUTTON_B,
            "x" => BUTTON_X,
            "y" => BUTTON_Y,
            "z" => TRIGGER_Z,
            "l" => TRIGGER_L,
            "r" => TRIGGER_R,
            "start" => BUTTON_START,
            "up" => BUTTON_UP,
            "down" => BUTTON_DOWN,
            "left" => BUTTON_LEFT,
            "right" => BUTTON_RIGHT,
            _ => return Err(format!("--holds: no button {name}")),
        };
    }
    Ok(Hold {
        port,
        button,
        sticks,
        from: from.parse().map_err(|_| bad())?,
        to: to.parse().map_err(|_| bad())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_holds_its_buttons() {
        let h = Holds::parse("none@0-9,start@10-12,a+p2@11-11,sx=-80@20-21").unwrap();
        assert_eq!(h.at(5)[0].button, 0);
        assert_eq!(h.at(10)[0].button, BUTTON_START);
        assert_eq!(h.at(11)[1].button, BUTTON_A);
        assert_eq!(h.at(20)[0].stick_x, -80);
        assert!(h.at(30).iter().all(|p| p.err == ERR_NONE && p.button == 0));
        assert!(Holds::parse("jump@1-2").is_err());
    }
}
