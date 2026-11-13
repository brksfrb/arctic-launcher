//! Sealed parent-selection outcomes within one ActorScope. Child/model/AST
//! identities are scalar comparison keys only, never dereferenced or serialized.
#[derive(Clone, PartialEq)]
pub(crate) struct Source {
    pub serial: u64,
    pub child: usize,
    pub model: usize,
    pub slot: u32,
    pub hierarchy: u32,
    pub bone: u32,
    pub expression: usize,
    pub source_hierarchy: u32,
    pub source_bone: u32,
    pub source_hash: u64,
    pub matrix: [f32; 16],
}
struct Setup {
    serial: u64,
    child: usize,
    model: usize,
    frame: u64,
    reload: u64,
}
#[derive(Default)]
pub(crate) struct Ledger {
    serial: u64,
    setups: [Option<Setup>; 7],
    sources: Vec<Source>,
    disabled: bool,
}
impl Ledger {
    pub(crate) fn invalidate(&mut self) {
        self.sources.clear();
        for setup in &mut self.setups {
            *setup = None;
        }
    }
    pub(crate) fn invalidate_slot(&mut self, slot: u32) {
        if let Some(setup) = self.setups.get_mut(slot as usize) {
            *setup = None;
        }
        self.sources.retain(|source| source.slot != slot);
    }
    pub(crate) fn begin(&mut self, slot: u32) -> Option<u64> {
        let index = usize::try_from(slot).ok()?;
        self.setups.get_mut(index)?;
        if self.disabled {
            return None;
        }
        let Some(serial) = self.serial.checked_add(1) else {
            self.disabled = true;
            self.sources.clear();
            return None;
        };
        self.serial = serial;
        self.sources.retain(|source| source.slot != slot);
        self.setups[index] = Some(Setup {
            serial,
            child: 0,
            model: 0,
            frame: 0,
            reload: 0,
        });
        Some(serial)
    }
    pub(crate) fn finish(
        &mut self,
        slot: u32,
        serial: u64,
        child: usize,
        model: usize,
        frame: u64,
        reload: u64,
        sources: Vec<Source>,
    ) -> bool {
        if self.disabled
            || child < 0x10000
            || model < 0x10000
            || sources.len() > 512
            || !self
                .setups
                .get(slot as usize)
                .and_then(Option::as_ref)
                .is_some_and(|setup| setup.serial == serial && setup.child == 0 && setup.model == 0)
        {
            return false;
        }
        if sources.iter().any(|source| {
            source.serial != serial
                || source.child != child
                || source.model != model
                || source.slot != slot
                || source.bone >= 256
                || source.source_bone >= 256
                || source.expression < 0x10000
                || !crate::native_emote_history::valid_pose(&crate::native_emote_history::Pose {
                    native: source.matrix,
                    supplied: source.matrix,
                    emote: None,
                    replay: None,
                })
        }) || self
            .sources
            .len()
            .checked_add(sources.len())
            .is_none_or(|count| count > 512)
        {
            return false;
        }
        let mut keys = std::collections::HashSet::new();
        if sources
            .iter()
            .any(|source| !keys.insert((source.hierarchy, source.bone)))
        {
            return false;
        }
        self.sources.extend(sources);
        self.setups[slot as usize] = Some(Setup {
            serial,
            child,
            model,
            frame,
            reload,
        });
        true
    }
    fn setup(&self, child: usize, model: usize, reload: u64) -> Option<&Setup> {
        if self.disabled || child < 0x10000 || model < 0x10000 {
            return None;
        }
        let mut matches =
            self.setups.iter().flatten().filter(|setup| {
                setup.child == child && setup.model == model && setup.reload == reload
            });
        let setup = matches.next()?;
        matches.next().is_none().then_some(setup)
    }
    pub(crate) fn frame(&self, child: usize, model: usize, reload: u64) -> Option<u64> {
        Some(self.setup(child, model, reload)?.frame)
    }
    pub(crate) fn serial(&self, child: usize, model: usize, reload: u64) -> Option<u64> {
        Some(self.setup(child, model, reload)?.serial)
    }
    pub(crate) fn source(
        &self,
        child: usize,
        model: usize,
        reload: u64,
        frame: u64,
        hierarchy: u32,
        bone: u32,
        expression: usize,
    ) -> Option<Source> {
        let setup = self.setup(child, model, reload)?;
        if setup.frame != frame {
            return None;
        }
        let mut matches = self.sources.iter().filter(|source| {
            source.serial == setup.serial
                && source.child == child
                && source.model == model
                && source.hierarchy == hierarchy
                && source.bone == bone
                && source.expression == expression
        });
        let source = matches.next()?;
        matches.next().is_none().then(|| source.clone())
    }
}
