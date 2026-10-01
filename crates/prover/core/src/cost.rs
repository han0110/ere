use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    io::Write,
    iter,
    ops::Range,
};

use elf::{ElfBytes, ParseError, abi::STT_FUNC, endian::AnyEndian};
use flate2::{Compression, write::GzEncoder};
use prost::Message;

use crate::{error::CommonError, pprof};

/// Cost of one execution.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CostEstimation {
    /// Cost per component. Each zkVM defines the unit.
    pub cost: BTreeMap<String, u64>,
}

/// Cost of one execution per guest call stack, and its peak memory use.
#[derive(Clone, Debug, PartialEq)]
pub struct CostProfile {
    /// A sample has one value per [`CostEstimation::cost`] component, then their `total`.
    pub pprof: pprof::Profile,
    /// Span of the stack pointer `x2`, as [`PeakMemory::stack_pointer`] counts it.
    pub peak_stack_bytes: u64,
    /// Bytes of zkVM memory that the guest reads or writes, in whole 8-byte words (16-byte leaves
    /// on OpenVM). The stack and the static data count too.
    pub peak_heap_bytes: u64,
}

impl CostProfile {
    /// Cost per component, equal to the `execute_estimated_cost` result of the same input.
    pub fn cost_estimation(&self) -> CostEstimation {
        let components = &self.pprof.sample_type[..self.pprof.sample_type.len() - 1];
        let cost = components
            .iter()
            .enumerate()
            .map(|(index, sample_type)| {
                let cost = self
                    .pprof
                    .sample
                    .iter()
                    .map(|sample| sample.value[index] as u64)
                    .sum();
                (
                    self.pprof.string_table[sample_type.r#type as usize].clone(),
                    cost,
                )
            })
            .collect();
        CostEstimation { cost }
    }

    /// The gzipped pprof file that `go tool pprof` reads.
    pub fn to_pprof(&self) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&self.pprof.encode_to_vec()).unwrap();
        encoder.finish().unwrap()
    }
}

/// Frame of the code outside every function symbol.
const UNKNOWN_FRAME: &str = "[unknown]";

/// Node above the frame of the entry point. It has no function and spends no cost.
const ROOT: usize = 0;

/// Frames of a pprof sample at most. A deeper stack keeps its innermost frames, which bounds the
/// profile size for deep recursion.
const MAX_STACK_DEPTH: usize = 1024;

/// Guest call tree of one execution, with the cost per component that each frame spends itself.
///
/// A profiler steps the guest and, at each frame change, charges the cost growth since the last
/// change to the frame that ran. Frames are the sized function symbols of the ELF, and code outside
/// every symbol shows as `[unknown]`. [`Self::transfer`] decides the frame changes.
pub struct CallTree {
    functions: Functions,
    components: Vec<String>,
    /// Parent node and name index of each node.
    nodes: Vec<(usize, usize)>,
    children: HashMap<(usize, usize), usize>,
    /// Cost of each node, one value per component.
    costs: Vec<u64>,
    /// Running cost at the last charge.
    charged: Vec<u64>,
    /// Node of the frame that runs.
    node: usize,
    /// Address range of the function of `node`.
    range: Range<u64>,
    /// Return address and stack pointer of each call on the stack, outermost first. A call into
    /// code outside every symbol has no stack pointer, because it can switch stacks.
    calls: Vec<(u64, Option<u64>)>,
}

impl CallTree {
    /// Call tree that starts in the frame of the function at `pc_start`.
    pub fn new(elf: &[u8], pc_start: u64, components: &[&str]) -> Result<Self, CommonError> {
        let functions = Functions::from_elf(elf)
            .map_err(|err| CommonError::deserialize("ELF symbols", "elf", err))?;
        Ok(Self::with_functions(functions, pc_start, components))
    }

    fn with_functions(functions: Functions, pc_start: u64, components: &[&str]) -> Self {
        let mut call_tree = Self {
            functions,
            components: components.iter().map(|name| (*name).to_owned()).collect(),
            nodes: vec![(ROOT, 0)],
            children: HashMap::new(),
            costs: vec![0; components.len()],
            charged: vec![0; components.len()],
            node: ROOT,
            range: 0..0,
            calls: Vec::new(),
        };
        call_tree.enter(ROOT, pc_start);
        call_tree
    }

    /// Whether `pc` is in the function of the frame that runs.
    #[inline]
    pub fn contains(&self, pc: u64) -> bool {
        self.range.contains(&pc)
    }

    /// Start of each function and of each gap between functions, in address order.
    pub fn function_bounds(&self) -> impl Iterator<Item = u64> + '_ {
        self.functions.parts.iter().map(|(start, _)| *start)
    }

    /// Charges the growth of `running`, the cost per component so far, to the frame that runs.
    pub fn charge(&mut self, running: &[u64]) {
        self.charge_node(self.node, running);
    }

    /// Charges the growth of `running` to the root frame `name`, which has no function symbol.
    pub fn charge_root(&mut self, name: &str, running: &[u64]) {
        let name = self.functions.name_index(name);
        let node = self.child(ROOT, name);
        self.charge_node(node, running);
    }

    /// Moves to the frame that runs at `pc` after a jump, or after the PC leaves the function.
    /// `link` is the value that the jump writes to its link register, and `sp` is the stack pointer
    /// at the jump. `transfer` follows the return-address hints of `jalr` in the RISC-V
    /// unprivileged ISA.
    ///
    /// - A return goes to the innermost call with the same return address and stack pointer. A call
    ///   to the return address of the running frame also returns, as `jalr ra, 0(ra)` does.
    /// - Other jumps to a function start enter a frame when they link `ra` or `t0`, and replace it
    ///   otherwise, because NativeAOT jumps through `t0` as a scratch register.
    /// - A pop-then-push jump resumes the call whose return address it targets, as a coroutine swap
    ///   does. Otherwise it calls into code outside every symbol, or replaces the frame.
    /// - A return that matches no call keeps a frame whose function holds the target, as a jump
    ///   table does. Otherwise it unwinds to the caller that `nonlocal_return_frames` finds, or
    ///   replaces the caller.
    /// - Any other jump out of the function replaces the frame, as a tail call does.
    pub fn transfer(&mut self, transfer: Option<Transfer>, pc: u64, link: u64, sp: u64) {
        let parent = self.nodes[self.node].0;
        let returned = match transfer {
            Some(Transfer::Return) => self
                .calls
                .iter()
                .rposition(|call| call.0 == pc && call.1.is_none_or(|call_sp| call_sp == sp)),
            Some(Transfer::Call) => {
                (self.calls.last() == Some(&(pc, Some(sp)))).then(|| self.calls.len() - 1)
            }
            _ => None,
        };
        let transfer = match transfer {
            _ if returned.is_some() => Some(Transfer::Return),
            Some(Transfer::ReturnAndCall) if self.functions.starts(pc) => Some(Transfer::Call),
            Some(Transfer::Return) if self.functions.starts(pc) => None,
            transfer => transfer,
        };
        match transfer {
            Some(Transfer::Call) => self.call(pc, link, Some(sp)),
            Some(Transfer::Return) if parent != ROOT => {
                let (name, range) = self.functions.lookup(pc);
                if let Some(index) = returned {
                    self.unwind(self.calls.len() - index, range);
                } else if !self.contains(pc) {
                    match self.nonlocal_return_frames(name, sp) {
                        Some(frames) => self.unwind(frames, range),
                        None => {
                            // A return into a function that did not make the call replaces the
                            // caller.
                            self.calls.pop();
                            self.enter(self.nodes[parent].0, pc);
                        }
                    }
                }
            }
            Some(Transfer::ReturnAndCall) => {
                // Each coroutine has a stack of its own, so only the address has to match.
                let resumed = self.calls.iter().rposition(|call| call.0 == pc);
                if let Some(index) = resumed {
                    self.unwind(self.calls.len() - index, self.functions.lookup(pc).1);
                } else if self.functions.is_gap(pc) {
                    self.call(pc, link, None);
                } else {
                    self.enter(parent, pc);
                }
            }
            _ => self.enter(parent, pc),
        }
    }

    /// Frames that a nonlocal return such as `longjmp` to the function `name` with the stack
    /// pointer `sp` leaves. It returns to the innermost caller in `name` whose call ran with a
    /// higher stack pointer, else to the innermost caller in `name`.
    fn nonlocal_return_frames(&self, name: usize, sp: u64) -> Option<usize> {
        let mut caller = self.nodes[self.node].0;
        let mut frames = 1;
        let mut innermost = None;
        while caller != ROOT {
            if self.nodes[caller].1 == name {
                // The first frame has no call, so it holds every stack pointer.
                let call_sp = self
                    .calls
                    .len()
                    .checked_sub(frames + 1)
                    .and_then(|index| self.calls[index].1)
                    .unwrap_or(u64::MAX);
                if call_sp > sp {
                    return Some(frames);
                }
                innermost.get_or_insert(frames);
            }
            caller = self.nodes[caller].0;
            frames += 1;
        }
        innermost
    }

    /// Returns from `frames` frames into the function whose address range is `range`.
    fn unwind(&mut self, frames: usize, range: Range<u64>) {
        for _ in 0..frames {
            self.node = self.nodes[self.node].0;
        }
        let depth = self.calls.len() - frames;
        self.calls.truncate(depth);
        self.range = range;
    }

    /// Profile in `unit` with one sample per frame that spent any cost. A stack deeper than
    /// `MAX_STACK_DEPTH` frames keeps its innermost ones, and equal stacks then share a sample.
    pub fn into_profile(self, unit: &str, memory: &PeakMemory) -> CostProfile {
        let count = self.components.len();
        let mut string_table = vec![String::new(), unit.to_owned()];
        let sample_type: Vec<pprof::ValueType> = self
            .components
            .iter()
            .map(String::as_str)
            .chain(["total"])
            .map(|name| {
                string_table.push(name.to_owned());
                pprof::ValueType {
                    r#type: string_table.len() as i64 - 1,
                    unit: 1,
                }
            })
            .collect();
        let total = sample_type[count];

        // Each frame name gets one function and one location, whose id is their position in
        // `function` and `location` plus one.
        let mut ids = HashMap::new();
        let mut samples = HashMap::new();
        let (mut function, mut location, mut sample) = (Vec::new(), Vec::new(), Vec::new());
        for node in ROOT + 1..self.nodes.len() {
            let costs = &self.costs[node * count..][..count];
            if costs.iter().all(|cost| *cost == 0) {
                continue;
            }
            let mut location_id = Vec::new();
            let mut frame = node;
            while frame != ROOT && location_id.len() < MAX_STACK_DEPTH {
                let (parent, name) = self.nodes[frame];
                let id = *ids.entry(name).or_insert_with(|| {
                    string_table.push(self.functions.names[name].clone());
                    let id = function.len() as u64 + 1;
                    function.push(pprof::Function {
                        id,
                        name: string_table.len() as i64 - 1,
                        ..Default::default()
                    });
                    location.push(pprof::Location {
                        id,
                        mapping_id: 1,
                        line: vec![pprof::Line {
                            function_id: id,
                            ..Default::default()
                        }],
                        ..Default::default()
                    });
                    id
                });
                location_id.push(id);
                frame = parent;
            }
            // Only stacks at the depth limit can be equal, and then they share a sample.
            let index = *samples
                .entry(location_id)
                .or_insert_with_key(|location_id| {
                    sample.push(pprof::Sample {
                        location_id: location_id.clone(),
                        value: vec![0; count + 1],
                        label: Vec::new(),
                    });
                    sample.len() - 1
                });
            let value = costs.iter().copied().chain(iter::once(costs.iter().sum()));
            for (sum, cost) in sample[index].value.iter_mut().zip(value) {
                *sum += i64::try_from(cost).unwrap();
            }
        }

        CostProfile {
            pprof: pprof::Profile {
                sample_type,
                sample,
                mapping: vec![pprof::Mapping {
                    id: 1,
                    has_functions: true,
                    ..Default::default()
                }],
                location,
                function,
                string_table,
                period_type: Some(total),
                period: 1,
                default_sample_type: total.r#type,
                ..Default::default()
            },
            peak_stack_bytes: memory.peak_stack_bytes(),
            peak_heap_bytes: memory.peak_heap_bytes(),
        }
    }

    fn charge_node(&mut self, node: usize, running: &[u64]) {
        let costs = &mut self.costs[node * self.components.len()..][..self.components.len()];
        for ((cost, charged), running) in costs.iter_mut().zip(&mut self.charged).zip(running) {
            *cost += running
                .checked_sub(*charged)
                .expect("the running cost only grows");
            *charged = *running;
        }
    }

    fn enter(&mut self, parent: usize, pc: u64) {
        let (name, range) = self.functions.lookup(pc);
        self.node = self.child(parent, name);
        self.range = range;
    }

    /// Enters the frame of a call to `pc` that returns to `link` with the stack pointer `sp`, if
    /// known.
    fn call(&mut self, pc: u64, link: u64, sp: Option<u64>) {
        self.enter(self.node, pc);
        self.calls.push((link, sp));
    }

    fn child(&mut self, parent: usize, name: usize) -> usize {
        let next = self.nodes.len();
        let node = *self.children.entry((parent, name)).or_insert(next);
        if node == next {
            self.nodes.push((parent, name));
            self.costs.extend(iter::repeat_n(0, self.components.len()));
        }
        node
    }
}

/// Frame change of a jump, from the return-address hints of the RISC-V unprivileged ISA. The link
/// registers are `ra` and `t0`.
#[derive(Clone, Copy)]
pub enum Transfer {
    Call,
    Return,
    ReturnAndCall,
}

impl Transfer {
    /// Frame change of a `jal` or `jalr` that writes `rd` and jumps through `rs1`. A `jal` jumps
    /// through no register, so it passes its `rd` as `rs1`.
    pub fn of_jump(rd: u8, rs1: u8) -> Option<Self> {
        let is_link = |register| matches!(register, 1 | 5);
        match (is_link(rd), is_link(rs1)) {
            (true, true) if rd != rs1 => Some(Self::ReturnAndCall),
            (true, _) => Some(Self::Call),
            (false, true) => Some(Self::Return),
            (false, false) => None,
        }
    }
}

/// Function symbols of the guest ELF, as parts that cover the full address space.
struct Functions {
    /// Start address and name index of each part, in address order. A part ends where the next
    /// part starts.
    parts: Vec<(u64, usize)>,
    /// Demangled names. Index 0 is [`UNKNOWN_FRAME`], the name of each gap between functions.
    names: Vec<String>,
    name_indices: HashMap<String, usize>,
}

impl Functions {
    fn from_elf(elf: &[u8]) -> Result<Self, ParseError> {
        let elf = ElfBytes::<AnyEndian>::minimal_parse(elf)?;
        // A symbol table or a name that does not parse only loses its frame names.
        let symbols = match elf.symbol_table().ok().flatten() {
            Some((symbols, strings)) => symbols
                .iter()
                .filter(|symbol| symbol.st_symtype() == STT_FUNC && symbol.st_size > 0)
                .filter_map(|symbol| {
                    let name = strings.get(symbol.st_name as usize).ok()?;
                    Some((
                        symbol.st_value,
                        symbol.st_value.checked_add(symbol.st_size)?,
                        name,
                    ))
                })
                .collect(),
            None => Vec::new(),
        };
        Ok(Self::new(symbols))
    }

    fn new(mut symbols: Vec<(u64, u64, &str)>) -> Self {
        // An enclosing symbol sorts before a symbol that starts with it.
        symbols.sort_unstable_by_key(|&(start, end, name)| (start, Reverse(end), name));
        // An alias, or a symbol inside the previous function, adds no function.
        let mut kept: Vec<(u64, u64, String)> = Vec::new();
        for (start, end, name) in symbols {
            if kept
                .last()
                .is_none_or(|(_, last_end, _)| start >= *last_end)
            {
                kept.push((start, end, format!("{:#}", rustc_demangle::demangle(name))));
            }
        }
        let mut counts = HashMap::<&str, usize>::new();
        for (_, _, name) in &kept {
            *counts.entry(name).or_default() += 1;
        }

        let mut functions = Self {
            parts: vec![(0, 0)],
            names: Vec::new(),
            name_indices: HashMap::new(),
        };
        functions.name_index(UNKNOWN_FRAME);
        for (start, end, name) in &kept {
            // Distinct functions with one name, such as static functions of two C files, stay
            // apart.
            let index = if counts[name.as_str()] > 1 {
                functions.name_index(&format!("{name}@{start:#x}"))
            } else {
                functions.name_index(name)
            };
            if functions
                .parts
                .last()
                .is_some_and(|(last_start, _)| last_start == start)
            {
                functions.parts.pop();
            }
            functions.parts.push((*start, index));
            functions.parts.push((*end, 0));
        }
        functions
    }

    fn name_index(&mut self, name: &str) -> usize {
        if let Some(index) = self.name_indices.get(name) {
            return *index;
        }
        self.names.push(name.to_owned());
        self.name_indices
            .insert(name.to_owned(), self.names.len() - 1);
        self.names.len() - 1
    }

    /// Whether `pc` is the start of a function.
    fn starts(&self, pc: u64) -> bool {
        let (name, range) = self.lookup(pc);
        name != 0 && range.start == pc
    }

    /// Whether `pc` is outside every function symbol.
    fn is_gap(&self, pc: u64) -> bool {
        self.lookup(pc).0 == 0
    }

    /// Name index and address range of the part that holds `pc`.
    fn lookup(&self, pc: u64) -> (usize, Range<u64>) {
        let part = self.parts.partition_point(|(start, _)| *start <= pc) - 1;
        let end = self
            .parts
            .get(part + 1)
            .map_or(u64::MAX, |(start, _)| *start);
        (self.parts[part].1, self.parts[part].0..end)
    }
}

/// Bytes of memory that one bitmap page covers.
const PAGE_BYTES: u64 = 1 << 16;

/// Bitmap words of a page, one bit per 8-byte word of memory.
const PAGE_BITMAP_WORDS: usize = (PAGE_BYTES / 8 / 64) as usize;

/// Bitmap page of a first access, kept out of the hot path of [`PeakMemory::access`].
#[cold]
fn new_page() -> Box<[u64; PAGE_BITMAP_WORDS]> {
    Box::new([0; PAGE_BITMAP_WORDS])
}

/// Most that one `addi` lowers a register by, the reach of its 12-bit signed immediate.
const ADDI_REACH: u64 = 1 << 11;

/// How a step wrote the stack pointer `x2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackPointerWrite {
    /// The step loaded `x2` from memory.
    Load,
    /// The step wrote an upper immediate with `lui` or `auipc`.
    UpperImmediate,
    /// The step wrote another value to `x2`, or did not write it.
    Other,
}

/// Peak stack and heap use of one execution, from the stack pointer and the memory accesses.
pub struct PeakMemory {
    memory: Range<u64>,
    /// Touched 8-byte words of `memory`, in pages that the first access allocates.
    pages: Vec<Option<Box<[u64; PAGE_BITMAP_WORDS]>>>,
    sp: u64,
    /// Whether the step that wrote `sp` wrote an upper immediate.
    upper_immediate: bool,
    /// First and lowest value of the run of stack pointers that ends at `sp`, and whether a step
    /// kept one of them.
    run: (u64, u64, bool),
    stack_top: u64,
    stack_bottom: u64,
}

impl PeakMemory {
    /// Tracks accesses to the zkVM memory in `memory`.
    pub fn new(memory: Range<u64>) -> Self {
        let pages = (memory.end - memory.start).div_ceil(PAGE_BYTES) as usize;
        Self {
            memory,
            pages: vec![None; pages],
            sp: 0,
            upper_immediate: false,
            run: (0, 0, false),
            stack_top: 0,
            stack_bottom: u64::MAX,
        }
    }

    /// Records a read or write of `len` bytes at `address`.
    #[inline]
    pub fn access(&mut self, address: u64, len: u64) {
        let start = address.max(self.memory.start);
        let end = address.saturating_add(len).min(self.memory.end);
        if start >= end {
            return;
        }
        let words_per_page = PAGE_BYTES / 8;
        for word in (start - self.memory.start) / 8..=(end - 1 - self.memory.start) / 8 {
            let page = self.pages[(word / words_per_page) as usize].get_or_insert_with(new_page);
            let bit = word % words_per_page;
            page[(bit / 64) as usize] |= 1 << (bit % 64);
        }
    }

    /// Records the stack pointer after a step, and how the step wrote it. The peak stack is the
    /// span of the counted runs of values.
    ///
    /// - A lower value that is no load and no upper immediate continues the run. Any other new
    ///   value starts a run, and zero never counts.
    /// - A run counts from its first to its lowest value once a step keeps one of its values.
    /// - An upper immediate that the next step lowers within the reach of an `addi` is part of an
    ///   address, as in `auipc sp` and `addi sp, sp, -8`, so its run starts at the lower value.
    #[inline]
    pub fn stack_pointer(&mut self, sp: u64, write: StackPointerWrite) {
        if sp == self.sp {
            self.run.2 |= sp != 0;
        } else if sp > self.sp || write != StackPointerWrite::Other || sp == 0 {
            (self.stack_top, self.stack_bottom) = self.stack_span();
            self.run = (sp, sp, false);
        } else if self.upper_immediate
            && self.run == (self.sp, self.sp, false)
            // `addiw` completes the sign-extended value of a `lui` in the low 32 bits.
            && u64::from((self.sp as u32).wrapping_sub(sp as u32)) <= ADDI_REACH
        {
            self.run = (sp, sp, false);
        } else {
            self.run.1 = sp;
        }
        self.upper_immediate = write == StackPointerWrite::UpperImmediate;
        self.sp = sp;
    }

    /// Highest and lowest stack pointer with the run that ends at `sp`, when a step kept one of its
    /// values.
    fn stack_span(&self) -> (u64, u64) {
        let (first, lowest, kept) = self.run;
        if kept {
            (self.stack_top.max(first), self.stack_bottom.min(lowest))
        } else {
            (self.stack_top, self.stack_bottom)
        }
    }

    pub fn peak_stack_bytes(&self) -> u64 {
        let (top, bottom) = self.stack_span();
        top.saturating_sub(bottom)
    }

    pub fn peak_heap_bytes(&self) -> u64 {
        let words: u64 = self
            .pages
            .iter()
            .flatten()
            .flat_map(|page| page.iter())
            .map(|bits| u64::from(bits.count_ones()))
            .sum();
        8 * words
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use flate2::read::GzDecoder;
    use prost::Message;

    use crate::{
        cost::{
            CallTree, CostEstimation, Functions, MAX_STACK_DEPTH, PeakMemory, StackPointerWrite,
            Transfer, UNKNOWN_FRAME,
        },
        pprof,
    };

    #[test]
    fn call_tree_splits_cost_per_frame_and_component() {
        let functions = Functions::new(vec![(0x100, 0x200, "main"), (0x200, 0x300, "f")]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a", "b"]);
        call_tree.charge(&[1, 0]);
        call_tree.transfer(Transfer::of_jump(1, 1), 0x200, 0x150, 0x1000);
        call_tree.charge(&[3, 5]);
        call_tree.transfer(Transfer::of_jump(0, 1), 0x150, 0, 0x1000);
        call_tree.charge(&[4, 5]);
        call_tree.charge_root("[base]", &[4, 9]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let stack = |sample: &pprof::Sample| -> Vec<&str> {
            sample
                .location_id
                .iter()
                .rev()
                .map(|id| {
                    let function = &profile.pprof.function[*id as usize - 1];
                    profile.pprof.string_table[function.name as usize].as_str()
                })
                .collect()
        };
        let samples: Vec<(Vec<&str>, &[i64])> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (stack(sample), sample.value.as_slice()))
            .collect();
        assert_eq!(
            samples,
            [
                (vec!["main"], &[2, 0, 2][..]),
                (vec!["main", "f"], &[2, 5, 7][..]),
                (vec!["[base]"], &[0, 4, 4][..]),
            ]
        );
        assert_eq!(
            profile.cost_estimation(),
            CostEstimation {
                cost: [("a".to_owned(), 4), ("b".to_owned(), 9)].into(),
            }
        );

        let mut encoded = Vec::new();
        GzDecoder::new(profile.to_pprof().as_slice())
            .read_to_end(&mut encoded)
            .unwrap();
        assert_eq!(
            pprof::Profile::decode(encoded.as_slice()).unwrap(),
            profile.pprof
        );
    }

    #[test]
    fn call_tree_unwinds_nonlocal_returns_keeps_jump_tables_and_names_same_named_functions_apart() {
        let functions = Functions::new(vec![
            (0x100, 0x200, "main"),
            (0x200, 0x300, "f"),
            (0x300, 0x400, "helper"),
            (0x400, 0x500, "helper"),
            (0x500, 0x600, "memset"),
        ]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        for (running, pc, link, sp) in [
            (1, 0x200, 0x104, 0x1000),
            (2, 0x300, 0x204, 0xff0),
            (3, 0x400, 0x304, 0xfe0),
            (4, 0x400, 0x404, 0xfd0),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(Transfer::of_jump(1, 1), pc, link, sp);
        }
        // The innermost `helper` returns into `main`, past `f` and the other `helper` frames, as
        // `longjmp` does. Then `memset` jumps through `t0` into its own jump table and returns.
        // Last, `f` returns into `helper`, which did not call it, so `helper` replaces `main`.
        for (running, transfer, pc, link, sp) in [
            (5, Transfer::of_jump(0, 1), 0x150, 0, 0x1000),
            (6, Transfer::of_jump(1, 1), 0x500, 0x160, 0x1000),
            (7, Transfer::of_jump(0, 5), 0x580, 0, 0x1000),
            (8, Transfer::of_jump(0, 1), 0x160, 0, 0x1000),
            (9, Transfer::of_jump(1, 1), 0x200, 0x170, 0x1000),
            (10, Transfer::of_jump(0, 1), 0x350, 0, 0x1000),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(transfer, pc, link, sp);
        }
        call_tree.charge(&[11]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let stacks: Vec<String> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| {
                let names: Vec<&str> = sample
                    .location_id
                    .iter()
                    .rev()
                    .map(|id| {
                        let function = &profile.pprof.function[*id as usize - 1];
                        profile.pprof.string_table[function.name as usize].as_str()
                    })
                    .collect();
                names.join(";")
            })
            .collect();
        assert_eq!(
            stacks,
            [
                "main",
                "main;f",
                "main;f;helper@0x300",
                "main;f;helper@0x300;helper@0x400",
                "main;f;helper@0x300;helper@0x400;helper@0x400",
                "main;memset",
                "helper@0x300",
            ]
        );
        assert_eq!(profile.pprof.sample[0].value, [3, 3]);
        assert_eq!(profile.pprof.sample[5].value, [2, 2]);
    }

    #[test]
    fn call_tree_returns_to_the_frame_whose_call_returns_there() {
        let functions = Functions::new(vec![(0x100, 0x200, "main"), (0x200, 0x300, "f")]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        // `f` calls itself, jumps through `t0` into its own jump table, and returns twice.
        for (running, transfer, pc, link, sp) in [
            (1, Transfer::of_jump(1, 1), 0x200, 0x104, 0x1000),
            (2, Transfer::of_jump(1, 1), 0x200, 0x214, 0xff0),
            (3, Transfer::of_jump(0, 5), 0x280, 0, 0xfe0),
            (4, Transfer::of_jump(0, 1), 0x214, 0, 0xff0),
            (5, Transfer::of_jump(0, 1), 0x104, 0, 0x1000),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(transfer, pc, link, sp);
        }
        call_tree.charge(&[6]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let values: Vec<(usize, &[i64])> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (sample.location_id.len(), sample.value.as_slice()))
            .collect();
        assert_eq!(
            values,
            [(1, &[2, 2][..]), (2, &[2, 2][..]), (3, &[2, 2][..])]
        );
    }

    #[test]
    fn call_tree_keeps_a_frame_that_jumps_to_the_continuation_of_its_own_call() {
        let functions = Functions::new(vec![(0x100, 0x200, "main"), (0x200, 0x300, "f")]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        // The inner `f` jumps through `t0` to `0x214`, the continuation of the call that entered
        // it, with its own frame. Then it returns there with the stack pointer of that call.
        for (running, transfer, pc, link, sp) in [
            (1, Transfer::of_jump(1, 1), 0x200, 0x104, 0x1000),
            (3, Transfer::of_jump(1, 1), 0x200, 0x214, 0xff0),
            (6, Transfer::of_jump(0, 5), 0x214, 0, 0xfe0),
            (10, Transfer::of_jump(0, 1), 0x214, 0, 0xff0),
            (15, Transfer::of_jump(0, 1), 0x104, 0, 0x1000),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(transfer, pc, link, sp);
        }
        call_tree.charge(&[21]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let values: Vec<(usize, i64)> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (sample.location_id.len(), sample.value[0]))
            .collect();
        assert_eq!(values, [(1, 7), (2, 7), (3, 7)]);
    }

    #[test]
    fn call_tree_unwinds_a_nonlocal_return_to_the_instance_whose_stack_it_restores() {
        let functions = Functions::new(vec![
            (0x100, 0x200, "main"),
            (0x200, 0x300, "f"),
            (0x300, 0x400, "g"),
        ]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        // `f` calls itself, and the inner `f` calls `g`. Then `g` returns to `0x210` in `f` with
        // the stack pointer of the outer `f`, as `longjmp` does, and the outer `f` returns.
        for (running, transfer, pc, link, sp) in [
            (1, Transfer::of_jump(1, 1), 0x200, 0x104, 0x1000),
            (3, Transfer::of_jump(1, 1), 0x200, 0x224, 0xff0),
            (6, Transfer::of_jump(1, 1), 0x300, 0x244, 0xfe0),
            (10, Transfer::of_jump(0, 1), 0x210, 0, 0xff0),
            (15, Transfer::of_jump(0, 1), 0x104, 0, 0x1000),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(transfer, pc, link, sp);
        }
        call_tree.charge(&[21]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let values: Vec<(usize, i64)> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (sample.location_id.len(), sample.value[0]))
            .collect();
        assert_eq!(values, [(1, 7), (2, 7), (3, 3), (4, 4)]);
    }

    #[test]
    fn call_tree_calls_or_tail_calls_at_a_jump_to_a_function_start() {
        let functions = Functions::new(vec![
            (0x100, 0x200, "main"),
            (0x200, 0x300, "c"),
            (0x300, 0x400, "stub"),
            (0x400, 0x500, "m"),
            (0x500, 0x600, "n"),
        ]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        // `c` calls `stub`, which jumps through `t0` to the start of `m`, as an interface dispatch
        // stub does. Then `c` calls `n` with `jalr ra, 0(t0)`.
        for (running, transfer, pc, link, sp) in [
            (1, Transfer::of_jump(1, 1), 0x200, 0x104, 0x1000),
            (3, Transfer::of_jump(1, 1), 0x300, 0x210, 0xff0),
            (6, Transfer::of_jump(0, 5), 0x400, 0, 0xff0),
            (10, Transfer::of_jump(0, 1), 0x210, 0, 0xff0),
            (15, Transfer::of_jump(1, 5), 0x500, 0x220, 0xff0),
            (21, Transfer::of_jump(0, 1), 0x220, 0, 0xff0),
            (28, Transfer::of_jump(0, 1), 0x104, 0, 0x1000),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(transfer, pc, link, sp);
        }
        call_tree.charge(&[36]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let stacks: Vec<(String, i64)> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| {
                let names: Vec<&str> = sample
                    .location_id
                    .iter()
                    .rev()
                    .map(|id| {
                        let function = &profile.pprof.function[*id as usize - 1];
                        profile.pprof.string_table[function.name as usize].as_str()
                    })
                    .collect();
                (names.join(";"), sample.value[0])
            })
            .collect();
        assert_eq!(
            stacks,
            [
                ("main".to_owned(), 9),
                ("main;c".to_owned(), 14),
                ("main;c;stub".to_owned(), 3),
                ("main;c;m".to_owned(), 4),
                ("main;c;n".to_owned(), 6),
            ]
        );
    }

    #[test]
    fn call_tree_calls_code_outside_every_function_symbol_through_t0() {
        let functions = Functions::new(vec![(0x100, 0x200, "main"), (0x200, 0x300, "c")]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        // `c` calls code at `0x400`, which no function symbol covers, with `jalr ra, 0(t0)`. That
        // code returns to `c`, and `c` returns to `main`.
        for (running, transfer, pc, link, sp) in [
            (1, Transfer::of_jump(1, 1), 0x200, 0x104, 0x1000),
            (3, Transfer::of_jump(1, 5), 0x400, 0x210, 0xff0),
            (6, Transfer::of_jump(0, 1), 0x210, 0, 0xff0),
            (10, Transfer::of_jump(0, 1), 0x104, 0, 0x1000),
        ] {
            call_tree.charge(&[running]);
            call_tree.transfer(transfer, pc, link, sp);
        }
        call_tree.charge(&[15]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let stacks: Vec<(String, i64)> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| {
                let names: Vec<&str> = sample
                    .location_id
                    .iter()
                    .rev()
                    .map(|id| {
                        let function = &profile.pprof.function[*id as usize - 1];
                        profile.pprof.string_table[function.name as usize].as_str()
                    })
                    .collect();
                (names.join(";"), sample.value[0])
            })
            .collect();
        assert_eq!(
            stacks,
            [
                ("main".to_owned(), 6),
                ("main;c".to_owned(), 6),
                ("main;c;[unknown]".to_owned(), 3),
            ]
        );
    }

    #[test]
    fn call_tree_keeps_coroutine_swaps_without_function_symbols_bounded() {
        let mut call_tree = CallTree::with_functions(Functions::new(Vec::new()), 0x1000, &["a"]);
        // Coroutine `a` swaps to `b` with `jalr ra, 0(t0)`, and `b` swaps back with
        // `jalr t0, 0(ra)`, each to the continuation of the last swap of the other and each with a
        // stack pointer of its own. They swap 100 times.
        for swap in 0..50 {
            let (a, b) = (0x1000 + 0x10 * swap, 0x2000 + 0x10 * swap);
            let resume_b = if swap == 0 { 0x2000 } else { b - 0xc };
            call_tree.charge(&[2 * swap]);
            call_tree.transfer(Transfer::of_jump(1, 5), resume_b, a + 4, 0xb000);
            call_tree.charge(&[2 * swap + 1]);
            call_tree.transfer(Transfer::of_jump(5, 1), a + 4, b + 4, 0xa000);
        }
        call_tree.charge(&[100]);
        assert_eq!(call_tree.nodes.len(), 3);
        assert!(call_tree.calls.is_empty());
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let values: Vec<(usize, i64)> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (sample.location_id.len(), sample.value[0]))
            .collect();
        assert_eq!(values, [(1, 50), (2, 50)]);
    }

    #[test]
    fn call_tree_returns_at_a_call_to_the_return_address_of_the_running_frame() {
        // `c` calls `x`, which returns with `jalr ra, 0(ra)` in place of `ret`, as a NativeAOT
        // runtime helper does. The jump links `ra` again, so its hint is a call.
        let functions = Functions::new(vec![(0x100, 0x200, "c"), (0x200, 0x300, "x")]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        call_tree.charge(&[1]);
        call_tree.transfer(Transfer::of_jump(1, 12), 0x200, 0x108, 0x1000);
        call_tree.charge(&[2]);
        call_tree.transfer(Transfer::of_jump(1, 1), 0x108, 0x2fc, 0x1000);
        call_tree.charge(&[3]);
        assert!(call_tree.calls.is_empty());
        // The root, `c`, and `x` below `c`.
        assert_eq!(call_tree.nodes.len(), 3);
    }

    #[test]
    fn call_tree_returns_to_a_call_whose_return_address_starts_a_function() {
        // `f` ends with `jal h`, and the continuation of the call has a function symbol `g` of
        // its own. `g` jumps back to repeat the call. This repeats 100 times.
        let functions = Functions::new(vec![
            (0x100, 0x108, "f"),
            (0x108, 0x200, "g"),
            (0x200, 0x300, "h"),
        ]);
        let mut call_tree = CallTree::with_functions(functions, 0x104, &["a"]);
        for cycle in 0..100 {
            call_tree.charge(&[3 * cycle]);
            call_tree.transfer(Transfer::of_jump(1, 1), 0x200, 0x108, 0x1000);
            call_tree.charge(&[3 * cycle + 1]);
            call_tree.transfer(Transfer::of_jump(0, 1), 0x108, 0, 0x1000);
            call_tree.charge(&[3 * cycle + 2]);
            call_tree.transfer(Transfer::of_jump(0, 0), 0x104, 0, 0x1000);
        }
        call_tree.charge(&[300]);
        assert!(call_tree.calls.is_empty());
        // The root, `f`, and `h` below `f`.
        assert_eq!(call_tree.nodes.len(), 3);
    }

    #[test]
    fn call_tree_returns_from_code_without_function_symbols_on_another_stack() {
        let mut call_tree = CallTree::with_functions(Functions::new(Vec::new()), 0x1000, &["a"]);
        // Code without function symbols switches to another stack and calls with
        // `jalr ra, 0(t0)`. The callee switches back and returns with `ret`. This repeats 100
        // times.
        for cycle in 0..100 {
            let link = 0x1004 + 0x10 * cycle;
            call_tree.charge(&[2 * cycle]);
            call_tree.transfer(Transfer::of_jump(1, 5), 0x8000, link, 0xb000);
            call_tree.charge(&[2 * cycle + 1]);
            call_tree.transfer(Transfer::of_jump(0, 1), link, 0, 0xa000);
        }
        call_tree.charge(&[200]);
        assert_eq!(call_tree.nodes.len(), 3);
        assert!(call_tree.calls.is_empty());
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        let values: Vec<(usize, i64)> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (sample.location_id.len(), sample.value[0]))
            .collect();
        assert_eq!(values, [(1, 100), (2, 100)]);
    }

    #[test]
    fn call_tree_cuts_deep_stacks_and_merges_the_cut_ones() {
        let functions = Functions::new(vec![(0x100, 0x200, "f")]);
        let mut call_tree = CallTree::with_functions(functions, 0x100, &["a"]);
        // Each of the `MAX_STACK_DEPTH + 3` frames of a recursion spends 1.
        let depth = MAX_STACK_DEPTH as u64 + 3;
        for running in 1..depth {
            call_tree.charge(&[running]);
            call_tree.transfer(Transfer::of_jump(1, 1), 0x100, 0x104, 0x1000);
        }
        call_tree.charge(&[depth]);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0));

        // The 4 deepest frames share the stack of `MAX_STACK_DEPTH` frames.
        assert_eq!(profile.pprof.sample.len(), MAX_STACK_DEPTH);
        let deepest = profile.pprof.sample.last().unwrap();
        assert_eq!(deepest.location_id.len(), MAX_STACK_DEPTH);
        assert_eq!(deepest.value, [4, 4]);
        assert_eq!(profile.cost_estimation().cost["a"], depth);
    }

    #[test]
    fn functions_keep_the_enclosing_symbol_of_symbols_that_overlap() {
        let functions = Functions::new(vec![
            (0x100, 0x140, "f_fast"),
            (0x100, 0x200, "f"),
            (0x180, 0x1c0, "f_inner"),
            (0x200, 0x300, "g"),
            (0x200, 0x300, "g_alias"),
        ]);
        let lookup = |pc| {
            let (name, range) = functions.lookup(pc);
            (functions.names[name].as_str(), range)
        };
        assert_eq!(lookup(0x150), ("f", 0x100..0x200));
        assert_eq!(lookup(0x190), ("f", 0x100..0x200));
        assert_eq!(lookup(0x250), ("g", 0x200..0x300));
        assert_eq!(lookup(0x300), (UNKNOWN_FRAME, 0x300..u64::MAX));
    }

    #[test]
    fn peak_memory_counts_words_in_range_and_stack_pointers_that_the_program_runs_with() {
        let mut memory = PeakMemory::new(0x1000..0x20_1000);
        // The accesses touch two words across a page boundary and one word twice. An access that
        // starts before the memory counts only its word inside the memory, and an access past the
        // end counts nothing.
        memory.access(0x10_0ffc, 8);
        memory.access(0x1008, 1);
        memory.access(0x100c, 4);
        memory.access(0x0ff8, 16);
        memory.access(0x20_1000, 8);
        assert_eq!(memory.peak_heap_bytes(), 8 * 4);

        // `0x9000` and `0x8f08` compute the address that the top `0x2000` loads from, as the SP1
        // entry code does. `0x5000` is an address that the next load replaces. So none of them
        // counts.
        for (sp, write) in [
            (0, StackPointerWrite::Other),
            (0x9000, StackPointerWrite::Other),
            (0x8f08, StackPointerWrite::Other),
            (0x2000, StackPointerWrite::Load),
            (0x2000, StackPointerWrite::Other),
            (0x1c00, StackPointerWrite::Other),
            (0x1c00, StackPointerWrite::Other),
            (0x5000, StackPointerWrite::Other),
            (0x1800, StackPointerWrite::Load),
            (0x1800, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x800);

        // A top that the program reserves a frame on at once counts.
        let mut memory = PeakMemory::new(0..0);
        for sp in [0x4000, 0x3c00, 0x3c00] {
            memory.stack_pointer(sp, StackPointerWrite::Other);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x400);
    }

    #[test]
    fn peak_memory_counts_a_large_frame_and_a_value_that_a_restoring_load_leaves() {
        // The program reserves a 4 KiB frame with one instruction right after it sets the top.
        let mut memory = PeakMemory::new(0..0);
        for sp in [0x8000, 0x7000, 0x7000] {
            memory.stack_pointer(sp, StackPointerWrite::Other);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x1000);

        // The deepest value runs only the load that restores the stack pointer from its frame.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0x8000, StackPointerWrite::Other),
            (0x8000, StackPointerWrite::Other),
            (0x7ff0, StackPointerWrite::Other),
            (0x8000, StackPointerWrite::Load),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x10);
    }

    #[test]
    fn peak_memory_skips_an_upper_immediate_that_an_addi_completes() {
        // `auipc sp` and `addi sp, sp, -0x2f4` compute the top `0xa0400000`, as the zesu entry
        // code does, and the program then reserves a 0x100-byte frame.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0xa040_02f4, StackPointerWrite::UpperImmediate),
            (0xa040_0000, StackPointerWrite::Other),
            (0xa040_0000, StackPointerWrite::Other),
            (0xa03f_ff00, StackPointerWrite::Other),
            (0xa03f_ff00, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x100);

        // `auipc sp` and `addi sp, sp, -0x800` lower the upper immediate by the whole reach of an
        // `addi`, so the run starts at the lower value.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0x8800, StackPointerWrite::UpperImmediate),
            (0x8000, StackPointerWrite::Other),
            (0x8000, StackPointerWrite::Other),
            (0x7f00, StackPointerWrite::Other),
            (0x7f00, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x100);

        // A whole top that `lui` writes counts, because a step keeps it.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0x8000, StackPointerWrite::UpperImmediate),
            (0x8000, StackPointerWrite::Other),
            (0x7f00, StackPointerWrite::Other),
            (0x7f00, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x100);

        // `lui sp, 0x80000` and `addiw sp, sp, -16` compute the top `0x7ffffff0`. The `lui`
        // value is sign-extended, so `addiw` lowers it within reach only in the low 32 bits.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0xffff_ffff_8000_0000, StackPointerWrite::UpperImmediate),
            (0x7fff_fff0, StackPointerWrite::Other),
            (0x7fff_fff0, StackPointerWrite::Other),
            (0x7fff_fef0, StackPointerWrite::Other),
            (0x7fff_fef0, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x100);

        // A whole top that `lui` writes counts when the next step lowers it past the reach of an
        // `addi`, as `sub sp, sp, t0` reserves a 4 KiB frame.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0x8000, StackPointerWrite::UpperImmediate),
            (0x7000, StackPointerWrite::Other),
            (0x7000, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x1000);

        // An upper immediate that writes the kept value again leaves the run as it is.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0x9000, StackPointerWrite::Other),
            (0x9000, StackPointerWrite::Other),
            (0x9000, StackPointerWrite::UpperImmediate),
            (0x8ff0, StackPointerWrite::Other),
            (0x8ff0, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x10);

        // A `lui` starts a new run, so the partial value of a switch to another stack does not
        // count.
        let mut memory = PeakMemory::new(0..0);
        for (sp, write) in [
            (0x8000, StackPointerWrite::Other),
            (0x8000, StackPointerWrite::Other),
            (0x7f00, StackPointerWrite::Other),
            (0x7f00, StackPointerWrite::Other),
            (0x1000, StackPointerWrite::UpperImmediate),
            (0x9000, StackPointerWrite::Other),
            (0x9000, StackPointerWrite::Other),
        ] {
            memory.stack_pointer(sp, write);
        }
        assert_eq!(memory.peak_stack_bytes(), 0x1100);
    }
}
