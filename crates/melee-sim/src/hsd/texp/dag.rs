//! Scheduling and simplifying TEV expressions (`sysdolphin/baselib/texpdag.c`): the graph of
//! operations a compile turns into stages, the order with the fewest registers, and the
//! rewrites that fold pass-through and mergeable operations into their users.

use super::gx::*;
use super::{
    Arg, Exp, MAX_NUM, Res, TE_0, TE_1, TE_A, TE_RGB, TExpList, TY_CNST, TY_IMM, TY_KONST, TY_RAS,
    TY_TEV, TY_TEX, TY_ZERO, is_through_alpha, is_through_color,
};

/// `HSD_TExpDag`: an operation and the operations it reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dag {
    /// The operation (`tev`), by its expression.
    pub tev: usize,
    pub idx: u8,
    pub nb_dep: u8,
    pub nb_ref: u8,
    /// Entries of the list this one reads.
    pub depend: [usize; 8],
}

/// `make_dependancy_mtx`: each entry's direct dependencies as bits.
fn make_dependency_mtx(list: &[Dag], dep: &mut [u32]) {
    for (i, d) in list.iter().enumerate() {
        dep[i] = 0;
        for &j in &d.depend[..usize::from(d.nb_dep)] {
            dep[i] |= 1 << list[j].idx;
        }
    }
}

/// `make_full_dependancy_mtx`: the dependencies' transitive closure.
#[doc(alias = "make_full_dependancy_mtx")]
fn make_full_dependency_mtx(num: usize, dep: &[u32], full: &mut [u32]) {
    full[..num].copy_from_slice(&dep[..num]);
    loop {
        let mut changed = false;
        for j in 0..num {
            let flag = 1u32 << j;
            let bits = full[j];
            for f in &mut full[..num] {
                if flag & *f != 0 {
                    let old = *f;
                    *f |= bits;
                    if old != *f {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

impl TExpList {
    /// `CalcDistance`: each operation's greatest depth below `tev`.
    #[doc(alias = "CalcDistance")]
    fn calc_distance(&self, tevs: &[usize], dist: &mut [i32], tev: usize, depth: i32) {
        let Some(idx) = tevs.iter().position(|&t| t == tev) else {
            return;
        };
        if dist[idx] < depth {
            dist[idx] = depth;
            let t = *self.tev(tev);
            for i in 0..4 {
                if t.c_in[i].ty == TY_TEV {
                    self.calc_distance(tevs, dist, t.c_in[i].exp.node(), depth + 1);
                }
                if t.a_in[i].ty == TY_TEV {
                    self.calc_distance(tevs, dist, t.a_in[i].exp.node(), depth + 1);
                }
            }
        }
    }

    /// Lists the operations `root` reads, nearest first, with their dependencies,
    /// `HSD_TExpMakeDag`. Returns how many there are.
    #[doc(alias = "HSD_TExpMakeDag")]
    pub fn make_dag(&self, root: Exp, list: &mut Vec<Dag>) -> usize {
        assert_eq!(self.get_type(root), TY_TEV);
        let mut base = [0usize; MAX_NUM];
        base[0] = root.node();
        let mut num = 1;
        let mut j = 0;
        while j < num {
            let t = *self.tev(base[j]);
            for ins in [&t.c_in, &t.a_in] {
                for a in ins {
                    if a.ty == TY_TEV && !base[..num].contains(&a.exp.node()) {
                        assert!(num < MAX_NUM);
                        base[num] = a.exp.node();
                        num += 1;
                    }
                }
            }
            j += 1;
        }
        let base = &mut base[..num];
        let mut dist = [-1; MAX_NUM];
        let dist = &mut dist[..num];
        self.calc_distance(base, dist, base[0], 0);
        // The original's sort: one pass from each index, swapping neighbors out of order.
        for idx in 0..num {
            for j in idx + 1..num {
                if dist[j - 1] > dist[j] {
                    base.swap(j - 1, j);
                    dist.swap(j - 1, j);
                }
            }
        }

        list.clear();
        list.resize(num, Dag::default());
        for j in (0..num).rev() {
            let t = *self.tev(base[j]);
            let mut d = Dag {
                tev: base[j],
                idx: j as u8,
                ..Dag::default()
            };
            for ins in [&t.c_in, &t.a_in] {
                for a in ins {
                    if a.ty != TY_TEV {
                        continue;
                    }
                    let l = (j..num)
                        .find(|&l| base[l] == a.exp.node())
                        .expect("an operation reading one listed before it");
                    if !d.depend[..usize::from(d.nb_dep)].contains(&l) {
                        d.depend[usize::from(d.nb_dep)] = l;
                        d.nb_dep += 1;
                        list[l].nb_ref += 1;
                    }
                }
            }
            d.nb_ref = list[j].nb_ref;
            list[j] = d;
        }
        num
    }

    /// Gives each operation of an order its output registers, the last first, and returns how
    /// many color and alpha registers it took, `assign_reg`. It leaves its choices in the
    /// operations, so the last order tried keeps them.
    #[doc(alias = "assign_reg")]
    fn assign_dst(&mut self, num: usize, list: &[Dag], order: &[usize]) -> i32 {
        let mut color_refs = [0u8; 4];
        let mut alpha_refs = [0u8; 4];
        let mut min_color_reg = 4;
        let mut min_alpha_reg = 4;
        // A destination is 0xFF where none was found; the original would index past the
        // counts, which no material reaches.
        let dec = |refs: &mut [u8; 4], dst: u8| {
            if let Some(r) = refs.get_mut(usize::from(dst)) {
                *r = r.wrapping_sub(1);
            }
        };
        for idx in (0..num).rev() {
            let t = list[order[idx]].tev;
            let tev = *self.tev(t);
            for i in 0..4 {
                let c = tev.c_in[i];
                if self.get_type(c.exp) == TY_TEV {
                    let src = self.tev(c.exp.node());
                    if c.sel == TE_RGB {
                        dec(&mut color_refs, src.c_dst);
                    } else {
                        dec(&mut alpha_refs, src.a_dst);
                    }
                }
                let a = tev.a_in[i];
                if self.get_type(a.exp) == TY_TEV {
                    dec(&mut alpha_refs, self.tev(a.exp.node()).a_dst);
                }
            }
            let tev = self.tev_mut(t);
            if tev.c_ref > 0
                && let Some(i) = (0..4).rev().find(|&i| color_refs[i] == 0)
            {
                color_refs[i] = tev.c_ref as u8;
                tev.c_dst = i as u8;
                min_color_reg = min_color_reg.min(i as i32);
            }
            if tev.a_ref > 0
                && let Some(i) = (0..4).rev().find(|&i| alpha_refs[i] == 0)
            {
                alpha_refs[i] = tev.a_ref as u8;
                tev.a_dst = i as u8;
                min_alpha_reg = min_alpha_reg.min(i as i32);
            }
        }
        (4 - min_color_reg) + (4 - min_alpha_reg)
    }

    /// Tries every order of the operations their dependencies allow, keeping the one with the
    /// fewest registers below `min`, `order_dag`.
    #[allow(clippy::too_many_arguments)]
    fn order_dag(
        &mut self,
        num: usize,
        dep: &[u32],
        full_dep: &[u32],
        list: &[Dag],
        mut depth: usize,
        idx: usize,
        mut ready_set: u32,
        order: &mut [usize],
        min: &mut i32,
        min_order: &mut [usize],
    ) {
        ready_set &= !(1 << idx);
        order[depth] = usize::from(idx as u8);
        depth += 1;
        if depth == num {
            let score = self.assign_dst(num, list, order);
            if score < *min {
                *min = score;
                min_order[..num].copy_from_slice(&order[..num]);
            }
            return;
        }
        let dep_bits = dep[idx];
        let blocked = ready_set | dep_bits;
        let full_bits = (0..num)
            .filter(|&i| blocked & (1 << i) != 0)
            .fold(0, |f, i| f | full_dep[i]);
        let ready_set = blocked & !full_bits;
        let d = &list[idx];
        if d.nb_dep == 1 && ready_set & dep_bits != 0 {
            let next = usize::from(list[d.depend[0]].idx);
            self.order_dag(
                num, dep, full_dep, list, depth, next, ready_set, order, min, min_order,
            );
        } else {
            for i in 0..num {
                if ready_set & (1 << i) != 0 {
                    self.order_dag(
                        num, dep, full_dep, list, depth, i, ready_set, order, min, min_order,
                    );
                }
            }
        }
    }

    /// Orders the operations (`result`, users first) and points their inputs at the registers
    /// their sources write, `HSD_TExpSchedule`.
    #[doc(alias = "HSD_TExpSchedule")]
    pub fn schedule(&mut self, num: usize, list: &[Dag], result: &mut [usize], res: &mut Res) {
        const C_IN: [u32; 4] = [GX_CC_C0, GX_CC_C1, GX_CC_C2, GX_CC_CPREV];
        const A_IN: [u32; 4] = [GX_CC_A0, GX_CC_A1, GX_CC_A2, GX_CC_APREV];
        const ARGS: [u32; 4] = [GX_CA_A0, GX_CA_A1, GX_CA_A2, GX_CA_APREV];
        let mut dep = [0u32; MAX_NUM];
        let mut full = [0u32; MAX_NUM];
        let mut order = [0usize; MAX_NUM];
        let mut min_order = [0usize; MAX_NUM];
        // An order needing five or more registers is never kept, leaving every entry the root.
        let mut min = 5;
        make_dependency_mtx(&list[..num], &mut dep);
        make_full_dependency_mtx(num, &dep, &mut full);
        self.order_dag(
            num,
            &dep,
            &full,
            list,
            0,
            0,
            0,
            &mut order,
            &mut min,
            &mut min_order,
        );

        for i in 0..num {
            let t = list[min_order[i]].tev;
            result[i] = t;
            let tev = *self.tev(t);
            if tev.c_dst != 0xFF {
                res.reg[usize::from(tev.c_dst) + 4].color = 3;
                for j in 0..4 {
                    let c = tev.c_in[j];
                    if self.get_type(c.exp) == TY_TEV {
                        let src = *self.tev(c.exp.node());
                        let arg = if c.sel == TE_RGB {
                            C_IN[usize::from(src.c_dst)]
                        } else {
                            A_IN[usize::from(src.a_dst)]
                        };
                        self.tev_mut(t).c_in[j].arg = arg as u8;
                    }
                }
            }
            if tev.a_dst != 0xFF {
                res.reg[usize::from(tev.a_dst) + 4].alpha = 1;
                for j in 0..4 {
                    let a = tev.a_in[j];
                    if self.get_type(a.exp) == TY_TEV {
                        let src = *self.tev(a.exp.node());
                        self.tev_mut(t).a_in[j].arg = ARGS[usize::from(src.a_dst)] as u8;
                    }
                }
            }
        }
    }

    /// Simplifies an operation's sources and reads through those that pass a value on,
    /// `SimplifySrc`.
    #[doc(alias = "SimplifySrc")]
    fn simplify_src(&mut self, t: usize) -> bool {
        let mut result = false;
        for i in 0..4 {
            let a = self.tev(t).c_in[i];
            if a.ty != TY_TEV {
                continue;
            }
            let (src, sel) = (a.exp, a.sel);
            if self.simplify(src) {
                result = true;
            }
            let s = *self.tev(src.node());
            if sel == TE_RGB {
                match s.c_op {
                    0xFF => {
                        self.unref(src, sel);
                        result = true;
                        self.tev_mut(t).c_in[i] = Arg::CLEARED;
                    }
                    0 if s.c_in[0].sel == TE_0
                        && s.c_in[1].sel == TE_0
                        && s.c_bias == 0
                        && s.c_scale == 0 =>
                    {
                        let d3 = s.c_in[3];
                        match d3.ty {
                            TY_TEV => {
                                if self.tev(d3.exp.node()).c_clamp != 0 || s.c_clamp == 0 {
                                    self.tev_mut(t).c_in[i] = d3;
                                    self.ref_(d3.exp, d3.sel);
                                    self.unref(src, sel);
                                    result = true;
                                }
                            }
                            TY_TEX => {
                                let d = self.tev_mut(t);
                                if (d.tex.is_none() || d.tex == s.tex)
                                    && (d.tex_swap == 0xFF
                                        || s.tex_swap == 0xFF
                                        || d.tex_swap == s.tex_swap)
                                {
                                    d.c_in[i] = d3;
                                    d.tex = s.tex;
                                    if d.tex_swap == 0xFF {
                                        d.tex_swap = s.tex_swap;
                                    }
                                    self.unref(src, sel);
                                    result = true;
                                }
                            }
                            TY_RAS => {
                                let d = self.tev_mut(t);
                                // The original compares the rasterized swap with the source's
                                // texture swap, and takes the texture swap.
                                if (d.chan == 0xFF || d.chan == s.chan)
                                    && (d.ras_swap == 0xFF
                                        || s.ras_swap == 0xFF
                                        || d.ras_swap == s.tex_swap)
                                {
                                    d.c_in[i] = d3;
                                    d.chan = s.chan;
                                    if d.tex_swap == 0xFF {
                                        d.tex_swap = s.tex_swap;
                                    }
                                    self.unref(src, sel);
                                    result = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            } else if s.a_op == 0xFF {
                self.unref(src, sel);
                result = true;
                self.tev_mut(t).c_in[i] = Arg::CLEARED;
            }
        }

        for i in 0..4 {
            let a = self.tev(t).a_in[i];
            if a.ty != TY_TEV {
                continue;
            }
            let (src, sel) = (a.exp, a.sel);
            self.simplify(src);
            let s = *self.tev(src.node());
            match s.a_op {
                0xFF => {
                    self.unref(src, sel);
                    result = true;
                    self.tev_mut(t).a_in[i] = Arg::CLEARED;
                }
                0 if s.a_in[0].sel == TE_0
                    && s.a_in[1].sel == TE_0
                    && s.a_bias == 0
                    && s.a_scale == 0 =>
                {
                    let d3 = s.a_in[3];
                    match d3.ty {
                        TY_TEV => {
                            self.tev_mut(t).a_in[i] = d3;
                            self.ref_(d3.exp, d3.sel);
                            self.unref(src, sel);
                            result = true;
                        }
                        TY_TEX => {
                            let d = self.tev_mut(t);
                            if d.tex.is_none() || d.tex == s.tex {
                                d.a_in[i] = d3;
                                d.tex = s.tex;
                                self.unref(src, sel);
                                result = true;
                            }
                        }
                        TY_RAS => {
                            let d = self.tev_mut(t);
                            if d.chan == 0xFF || d.chan == s.chan {
                                d.a_in[i] = d3;
                                d.chan = s.chan;
                                self.unref(src, sel);
                                result = true;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        result
    }

    /// Drops a color input's use and zeroes it.
    fn drop_c(&mut self, t: usize, i: usize) {
        let a = self.tev(t).c_in[i];
        self.unref(a.exp, a.sel);
        self.tev_mut(t).c_in[i] = Arg::CLEARED;
    }

    /// Drops an alpha input's use and zeroes it.
    fn drop_a(&mut self, t: usize, i: usize) {
        let a = self.tev(t).a_in[i];
        self.unref(a.exp, a.sel);
        self.tev_mut(t).a_in[i] = Arg::CLEARED;
    }

    /// Removes an operation's unused outputs and inputs its operation ignores, `SimplifyThis`.
    #[doc(alias = "SimplifyThis")]
    fn simplify_this(&mut self, t: usize) -> bool {
        let mut result = false;
        loop {
            let tev = self.tev_mut(t);
            let any = |ins: &[Arg; 4], ty| ins.iter().any(|a| a.ty == ty);
            if !any(&tev.c_in, TY_TEX) && !any(&tev.a_in, TY_TEX) {
                tev.tex = None;
                tev.tex_swap = 0xFF;
            }
            if !any(&tev.c_in, TY_RAS) && !any(&tev.a_in, TY_RAS) {
                tev.chan = 0xFF;
                tev.ras_swap = 0xFF;
            }

            let mut changed = false;
            let a_op = self.tev(t).a_op;
            if a_op == 0xFF || a_op == 0xE || a_op == 0xF || a_op <= 1 {
                if self.tev(t).c_op != 0xFF && self.tev(t).c_ref == 0 {
                    self.tev_mut(t).c_op = 0xFF;
                    for i in 0..4 {
                        self.drop_c(t, i);
                    }
                    changed = true;
                }
                let sel = |s: &Self, i: usize| s.tev(t).c_in[i].sel;
                match self.tev(t).c_op {
                    0 | 1 => {
                        if sel(self, 2) == TE_0 {
                            if sel(self, 1) != TE_0 {
                                self.drop_c(t, 1);
                                changed = true;
                            }
                            if self.tev(t).c_op == 0 && sel(self, 3) == TE_0 {
                                changed = true;
                                let d = self.tev_mut(t);
                                d.c_in[3] = d.c_in[0];
                                d.c_in[0] = Arg::CLEARED;
                                d.c_clamp = 1;
                            }
                        }
                        if sel(self, 2) == TE_1 {
                            if sel(self, 0) != TE_0 {
                                self.drop_c(t, 0);
                                changed = true;
                            }
                            if self.tev(t).c_op == 0 && sel(self, 3) == TE_0 {
                                changed = true;
                                let d = self.tev_mut(t);
                                d.c_in[3] = d.c_in[1];
                                d.c_in[1] = Arg::CLEARED;
                                d.c_in[2] = Arg::CLEARED;
                            }
                        }
                        if sel(self, 0) == TE_0
                            && sel(self, 1) == TE_0
                            && sel(self, 3) == TE_0
                            && self.tev(t).c_bias == 0
                        {
                            self.tev_mut(t).c_op = 0xFF;
                            self.drop_c(t, 2);
                            changed = true;
                        }
                    }
                    8 | 10 | 12 | 14 => {
                        if sel(self, 2) == TE_0 {
                            self.tev_mut(t).c_op = 0;
                            self.drop_c(t, 0);
                            self.drop_c(t, 1);
                            changed = true;
                        } else if sel(self, 0) == TE_0 {
                            self.tev_mut(t).c_op = 0;
                            self.drop_c(t, 1);
                            self.drop_c(t, 2);
                            changed = true;
                        }
                    }
                    9 | 11 | 13 | 15 => {
                        if sel(self, 2) == TE_0 {
                            self.tev_mut(t).c_op = 0;
                            self.drop_c(t, 0);
                            self.drop_c(t, 1);
                            changed = true;
                        } else if sel(self, 0) == TE_0 && sel(self, 1) == TE_0 {
                            self.tev_mut(t).c_op = 0;
                            changed = true;
                            let d = self.tev_mut(t);
                            d.c_in[0] = d.c_in[2];
                            d.c_in[2] = Arg::CLEARED;
                        }
                    }
                    _ => {}
                }
            }

            if self.tev(t).a_op != 0xFF && self.tev(t).a_ref == 0 {
                self.tev_mut(t).a_op = 0xFF;
                for i in 0..4 {
                    self.drop_a(t, i);
                }
                changed = true;
            }
            let sel = |s: &Self, i: usize| s.tev(t).a_in[i].sel;
            match self.tev(t).a_op {
                0 | 1 => {
                    if sel(self, 2) == TE_0 {
                        if sel(self, 1) != TE_0 {
                            self.drop_a(t, 1);
                            changed = true;
                        }
                        if self.tev(t).a_op == 0 && sel(self, 3) == TE_0 {
                            changed = true;
                            let d = self.tev_mut(t);
                            d.a_in[3] = d.a_in[0];
                            d.a_in[0] = Arg::CLEARED;
                        }
                    }
                    if sel(self, 2) == TE_1 {
                        if sel(self, 0) != TE_0 {
                            self.drop_a(t, 0);
                            changed = true;
                        }
                        if self.tev(t).a_op == 0 && sel(self, 3) == TE_0 {
                            changed = true;
                            let d = self.tev_mut(t);
                            d.a_in[3] = d.a_in[1];
                            d.a_in[1] = Arg::CLEARED;
                            d.a_in[2] = Arg::CLEARED;
                        }
                    }
                    // Unlike the color, the input `c` is left in place.
                    if sel(self, 0) == TE_0 && sel(self, 1) == TE_0 && sel(self, 3) == TE_0 {
                        self.tev_mut(t).a_op = 0xFF;
                        changed = true;
                    }
                }
                8..=0xD => {
                    if sel(self, 2) == TE_0 {
                        self.tev_mut(t).a_op = 0;
                        self.drop_a(t, 0);
                        self.drop_a(t, 1);
                        changed = true;
                    }
                }
                0xE => {
                    if sel(self, 2) == TE_0 {
                        self.tev_mut(t).a_op = 0;
                        self.drop_a(t, 0);
                        self.drop_a(t, 1);
                        changed = true;
                    } else if sel(self, 0) == TE_0 {
                        self.tev_mut(t).a_op = 0;
                        self.drop_a(t, 1);
                        self.drop_a(t, 2);
                        changed = true;
                    }
                }
                0xF => {
                    if sel(self, 2) == TE_0 {
                        self.tev_mut(t).a_op = 0;
                        self.drop_a(t, 0);
                        self.drop_a(t, 1);
                        changed = true;
                    } else if sel(self, 0) == TE_0 && sel(self, 1) == TE_0 {
                        self.tev_mut(t).a_op = 0;
                        changed = true;
                        let d = self.tev_mut(t);
                        d.a_in[0] = d.a_in[2];
                        d.a_in[2] = Arg::CLEARED;
                    }
                }
                _ => {}
            }

            if changed {
                result = true;
            } else {
                return result;
            }
        }
    }

    /// Whether two operations read different textures or channels (`RESOURCE_CONFLICT`).
    fn conflict(&self, t: usize, child: usize) -> bool {
        let (d, c) = (self.tev(t), self.tev(child));
        (d.tex.is_some() && c.tex.is_some() && d.tex != c.tex)
            || (d.chan != 0xFF && c.chan != 0xFF && d.chan != c.chan)
    }

    /// Folds a source that adds or subtracts into an operation that adds it to nothing,
    /// `SimplifyByMerge`.
    #[doc(alias = "SimplifyByMerge")]
    fn simplify_by_merge(&mut self, t: usize) -> bool {
        // `GX_TB_*` as a number, and back.
        let bias_int = |b: u8| match b {
            1 => 1,
            2 => -1,
            _ => 0,
        };
        let mut result = false;
        loop {
            let mut merged = false;
            let a_op = self.tev(t).a_op;
            if a_op == 0xFF || a_op == 0xE || a_op == 0xF || a_op <= 1 {
                let d = *self.tev(t);
                if (d.c_op == 0 || d.c_op == 1)
                    && d.c_in[1].sel == TE_0
                    && d.c_in[2].sel == TE_0
                    && self.get_type(d.c_in[0].exp) != TY_CNST
                    && self.get_type(d.c_in[3].exp) != TY_CNST
                {
                    if d.c_op == 0 && d.c_in[3].ty == TY_TEV {
                        let src = self.tev(d.c_in[3].exp.node());
                        let clamped = (d.c_in[3].sel == TE_RGB && src.c_clamp != 0)
                            || (d.c_in[3].sel == TE_A && src.a_clamp != 0);
                        if clamped && matches!(d.c_in[0].ty, TY_TEX | TY_RAS) {
                            self.tev_mut(t).c_in.swap(0, 3);
                        }
                    }
                    let d = *self.tev(t);
                    match d.c_in[0].ty {
                        TY_TEV if d.c_in[0].sel == TE_RGB => {
                            let (ce, child_sel) = (d.c_in[0].exp, d.c_in[0].sel);
                            let child = *self.tev(ce.node());
                            if (child.c_op == 0 || child.c_op == 1)
                                && child.c_in[3].sel == TE_0
                                && child.c_scale == 0
                                && !self.conflict(t, ce.node())
                            {
                                let mut bias = bias_int(child.c_bias);
                                if child.c_op == 1 {
                                    bias = -bias;
                                }
                                bias += bias_int(d.c_bias);
                                let new_bias = match bias {
                                    0 => Some(0),
                                    1 => Some(1),
                                    -1 => Some(2),
                                    _ => None,
                                };
                                merged = new_bias.is_some();
                                if let Some(b) = new_bias {
                                    let p = self.tev_mut(t);
                                    p.c_bias = b;
                                    if child.c_op == 1 {
                                        p.c_op = if p.c_op == 0 { 1 } else { 0 };
                                    }
                                    for i in 0..3 {
                                        self.tev_mut(t).c_in[i] = child.c_in[i];
                                        self.ref_(child.c_in[i].exp, child.c_in[i].sel);
                                    }
                                    let p = self.tev_mut(t);
                                    if p.tex.is_none() {
                                        p.tex = child.tex;
                                    }
                                    if p.chan == 0xFF {
                                        p.chan = child.chan;
                                    }
                                    if p.tex_swap == 0xFF {
                                        p.tex_swap = child.tex_swap;
                                    }
                                    if p.ras_swap == 0xFF {
                                        p.ras_swap = child.ras_swap;
                                    }
                                    self.unref(ce, child_sel);
                                }
                            }
                        }
                        TY_ZERO if d.c_in[3].ty == TY_TEV && d.c_in[3].sel == TE_RGB => {
                            let (ce, child_sel) = (d.c_in[3].exp, d.c_in[3].sel);
                            let child = *self.tev(ce.node());
                            if child.c_scale == 0
                                && (d.c_bias == 0 || d.c_bias != child.c_bias)
                                && !self.conflict(t, ce.node())
                            {
                                merged = true;
                                for i in 0..4 {
                                    self.tev_mut(t).c_in[i] = child.c_in[i];
                                    self.ref_(child.c_in[i].exp, child.c_in[i].sel);
                                }
                                let p = self.tev_mut(t);
                                p.c_op = child.c_op;
                                let mut bias = bias_int(child.c_bias);
                                if child.c_op == 1 {
                                    bias = -bias;
                                }
                                bias += bias_int(p.c_bias);
                                p.c_bias = match bias {
                                    1 => 1,
                                    -1 => 2,
                                    _ => 0,
                                };
                                if p.c_clamp == 0xFF || p.c_clamp == 0 {
                                    p.c_clamp = child.c_clamp;
                                }
                                if p.tex.is_none() {
                                    p.tex = child.tex;
                                }
                                if p.chan == 0xFF {
                                    p.chan = child.chan;
                                }
                                if p.tex_swap == 0xFF {
                                    p.tex_swap = child.tex_swap;
                                }
                                if p.ras_swap == 0xFF {
                                    p.ras_swap = child.ras_swap;
                                }
                                self.unref(ce, child_sel);
                            }
                        }
                        _ => {}
                    }
                }

                let d = *self.tev(t);
                if (d.a_op == 0 || d.a_op == 1)
                    && d.a_in[1].sel == TE_0
                    && d.a_in[2].sel == TE_0
                    && self.get_type(d.a_in[0].exp) != TY_CNST
                    && self.get_type(d.a_in[3].exp) != TY_CNST
                {
                    if d.a_op == 0
                        && d.a_in[3].ty == TY_TEV
                        && self.tev(d.a_in[3].exp.node()).a_clamp != 0
                        && matches!(d.a_in[0].ty, TY_TEX | TY_RAS)
                    {
                        self.tev_mut(t).a_in.swap(0, 3);
                    }
                    let d = *self.tev(t);
                    match d.a_in[0].ty {
                        TY_TEV => {
                            let (ce, child_sel) = (d.a_in[0].exp, d.a_in[0].sel);
                            let child = *self.tev(ce.node());
                            if (child.a_op == 0 || child.a_op == 1)
                                && child.a_in[3].sel == TE_0
                                && child.a_scale == 0
                                && !self.conflict(t, ce.node())
                            {
                                let mut bias = bias_int(child.a_bias);
                                if child.a_op == 1 {
                                    bias = -bias;
                                }
                                bias += bias_int(d.a_bias);
                                let new_bias = match bias {
                                    0 => Some(0),
                                    1 => Some(1),
                                    -1 => Some(2),
                                    _ => None,
                                };
                                merged = new_bias.is_some();
                                if let Some(b) = new_bias {
                                    let p = self.tev_mut(t);
                                    p.a_bias = b;
                                    if child.a_op == 1 {
                                        p.a_op = if p.a_op == 0 { 1 } else { 0 };
                                    }
                                    for i in 0..3 {
                                        self.tev_mut(t).a_in[i] = child.a_in[i];
                                        self.ref_(child.a_in[i].exp, child.a_in[i].sel);
                                    }
                                    let p = self.tev_mut(t);
                                    if p.tex.is_none() {
                                        p.tex = child.tex;
                                    }
                                    if p.chan == 0xFF {
                                        p.chan = child.chan;
                                    }
                                    self.unref(ce, child_sel);
                                }
                            }
                        }
                        TY_ZERO if d.a_in[3].ty == TY_TEV => {
                            let (ce, child_sel) = (d.a_in[3].exp, d.a_in[3].sel);
                            let child = *self.tev(ce.node());
                            if child.a_scale == 0
                                && (d.a_bias == 0 || d.a_bias != child.a_bias)
                                && !self.conflict(t, ce.node())
                            {
                                merged = true;
                                for i in 0..4 {
                                    self.tev_mut(t).a_in[i] = child.a_in[i];
                                    self.ref_(child.a_in[i].exp, child.a_in[i].sel);
                                }
                                let p = self.tev_mut(t);
                                p.a_op = child.a_op;
                                let mut bias = bias_int(child.a_bias);
                                if child.a_op == 1 {
                                    bias = -bias;
                                }
                                bias += bias_int(p.a_bias);
                                p.a_bias = match bias {
                                    1 => 1,
                                    -1 => 2,
                                    _ => 0,
                                };
                                if p.a_clamp == 0xFF || p.a_clamp == 0 {
                                    p.a_clamp = child.a_clamp;
                                }
                                if p.tex.is_none() {
                                    p.tex = child.tex;
                                }
                                if p.chan == 0xFF {
                                    p.chan = child.chan;
                                }
                                self.unref(ce, child_sel);
                            }
                        }
                        _ => {}
                    }
                }
            }
            if merged {
                result = true;
            } else {
                return result;
            }
        }
    }

    /// `HSD_TExpSimplify`.
    #[doc(alias = "HSD_TExpSimplify")]
    pub fn simplify(&mut self, e: Exp) -> bool {
        if self.get_type(e) != TY_TEV {
            return false;
        }
        let t = e.node();
        let mut res = false;
        if self.simplify_src(t) {
            res = true;
        }
        if self.simplify_this(t) {
            res = true;
        }
        if self.simplify_by_merge(t) {
            res = true;
        }
        res
    }

    /// Reads constants and immediates straight from sources that pass them through,
    /// `HSD_TExpSimplify2`.
    #[doc(alias = "HSD_TExpSimplify2")]
    pub fn simplify2(&mut self, t: usize) {
        for i in 0..4 {
            let a = self.tev(t).c_in[i];
            if a.ty != TY_TEV || a.sel != TE_RGB {
                continue;
            }
            let src = *self.tev(a.exp.node());
            if !is_through_color(&src) {
                continue;
            }
            let take = match src.c_in[3].ty {
                TY_KONST => {
                    let d = self.tev_mut(t);
                    if d.kcsel == 0xFF {
                        d.kcsel = src.kcsel;
                        true
                    } else {
                        d.kcsel == src.kcsel
                    }
                }
                TY_IMM => true,
                _ => false,
            };
            if take {
                let d3 = src.c_in[3];
                self.tev_mut(t).c_in[i] = d3;
                self.ref_(d3.exp, d3.sel);
                self.unref(a.exp, a.sel);
            }
        }
        for i in 0..4 {
            let a = self.tev(t).a_in[i];
            if a.ty != TY_TEV {
                continue;
            }
            let src = *self.tev(a.exp.node());
            if !is_through_alpha(&src) {
                continue;
            }
            let take = match src.a_in[3].ty {
                TY_KONST => {
                    let d = self.tev_mut(t);
                    if d.kasel == 0xFF {
                        d.kasel = src.kasel;
                        true
                    } else {
                        d.kasel == src.kasel
                    }
                }
                TY_IMM => true,
                _ => false,
            };
            if take {
                let d3 = src.a_in[3];
                self.tev_mut(t).a_in[i] = d3;
                self.ref_(d3.exp, d3.sel);
                self.unref(a.exp, a.sel);
            }
        }
    }
}
