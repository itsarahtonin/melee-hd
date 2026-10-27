//! The audio heap (`HSD_Synth_804D6018`, an OS heap): the device command queue's nodes and
//! the loaded sound banks' effect groups live in it. Where an allocation lands is game state:
//! a bank's header is read to the address its group's block got, and later allocations go
//! where frees left room. `OSAllocFromHeap` and `OSFreeToHeap` as Dolphin's OS has them:
//! first fit over the free cells in address order, a cell split when 64 bytes or more are
//! left, each cell a 32-byte header before its memory.

/// A cell: its address (its header's) and size, header included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Cell {
    pub at: u32,
    pub size: u32,
}

/// The heap's descriptor (`HeapDesc`): its size and its free and allocated cells, each list in
/// its order.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Heap {
    pub size: i32,
    /// In address order.
    pub free: Vec<Cell>,
    /// Most recent first.
    pub allocated: Vec<Cell>,
}

/// A cell's header.
pub const HEADER: u32 = 0x20;

impl Heap {
    /// `OSAllocFromHeap`: `size` bytes, or `None` when no free cell holds them.
    #[doc(alias = "OSAllocFromHeap")]
    pub fn alloc(&mut self, size: u32) -> Option<u32> {
        let size = (size + HEADER + 31) & !31;
        let i = self
            .free
            .iter()
            .position(|c| size as i32 <= c.size as i32)?;
        let cell = self.free[i];
        let left = cell.size.wrapping_sub(size);
        if left < 64 {
            self.free.remove(i);
            self.allocated.insert(0, cell);
        } else {
            self.free[i] = Cell {
                at: cell.at + size,
                size: left,
            };
            self.allocated.insert(0, Cell { at: cell.at, size });
        }
        Some(cell.at + HEADER)
    }

    /// `OSFreeToHeap`: the cell back among the free ones, joined with the free cells it
    /// touches.
    #[doc(alias = "OSFreeToHeap")]
    pub fn free(&mut self, ptr: u32) {
        let at = ptr - HEADER;
        let i = self
            .allocated
            .iter()
            .position(|c| c.at == at)
            .unwrap_or_else(|| panic!("OSFreeToHeap of {ptr:#x}, which the heap didn't give"));
        let mut cell = self.allocated.remove(i);
        // `DLInsert`: before the first free cell at or past it.
        let mut j = self
            .free
            .iter()
            .position(|c| cell.at <= c.at)
            .unwrap_or(self.free.len());
        if j < self.free.len() && cell.at + cell.size == self.free[j].at {
            cell.size += self.free[j].size;
            self.free.remove(j);
        }
        if j > 0 && self.free[j - 1].at + self.free[j - 1].size == cell.at {
            self.free[j - 1].size += cell.size;
            j -= 1;
            let _ = j;
            return;
        }
        self.free.insert(j, cell);
    }
}

impl super::Audio {
    /// `HSD_AudioMalloc`: `size` bytes of the audio heap; the original asserts there was room.
    #[doc(alias = "HSD_AudioMalloc")]
    pub fn audio_malloc(&mut self, size: u32) -> u32 {
        self.heap.alloc(size).expect("audio heap overflow.")
    }

    /// `HSD_AudioFree`: a block back to the audio heap.
    #[doc(alias = "HSD_AudioFree")]
    pub fn audio_free(&mut self, ptr: u32) {
        self.heap.free(ptr);
    }
}
