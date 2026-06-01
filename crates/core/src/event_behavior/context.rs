use super::runtime::append_music_delta;
use super::*;

pub struct CompileCtx<'a> {
    labels: &'a BTreeMap<String, u32>,
    pool: StringPool,
    flag_map: HashMap<String, u32>,
    var_map: HashMap<String, u32>,
}

impl<'a> CompileCtx<'a> {
    pub fn new(labels: &'a BTreeMap<String, u32>) -> Self {
        Self {
            labels,
            pool: StringPool::default(),
            flag_map: HashMap::new(),
            var_map: HashMap::new(),
        }
    }

    pub fn flag_count(&self) -> u32 {
        self.flag_map.len() as u32
    }

    pub fn intern(&mut self, value: &str) -> SharedStr {
        self.pool.intern(value)
    }

    pub fn resolve_target(&self, event_label: &str, target: &str) -> VnResult<u32> {
        self.labels.get(target).copied().ok_or_else(|| {
            VnError::InvalidScript(format!("{event_label} target '{target}' not found"))
        })
    }

    pub fn flag_id(&mut self, key: &str) -> VnResult<u32> {
        get_or_insert_id(&mut self.flag_map, key)
    }

    pub fn var_id(&mut self, key: &str) -> VnResult<u32> {
        get_or_insert_id(&mut self.var_map, key)
    }

    pub fn compile_cond(&mut self, cond: &CondRaw) -> VnResult<CondCompiled> {
        match cond {
            CondRaw::Flag { key, is_set } => {
                let flag_id = self.flag_id(key)?;
                Ok(CondCompiled::Flag {
                    flag_id,
                    is_set: *is_set,
                })
            }
            CondRaw::VarCmp { key, op, value } => {
                let var_id = self.var_id(key)?;
                Ok(CondCompiled::VarCmp {
                    var_id,
                    op: *op,
                    value: *value,
                })
            }
        }
    }
}

pub struct ExecutionCtx<'a> {
    pub(super) state: &'a mut EngineState,
    pub(super) script_events: &'a [EventCompiled],
    pub(super) audio_commands: &'a mut Vec<AudioCommand>,
    pub(super) read_dialogue_ips: &'a mut BTreeSet<u32>,
    pub(super) route_visited_ips: &'a mut BTreeSet<u32>,
    pub(super) pending_transition: &'a mut Option<SceneTransitionCompiled>,
}

impl<'a> ExecutionCtx<'a> {
    pub fn new(
        state: &'a mut EngineState,
        script_events: &'a [EventCompiled],
        audio_commands: &'a mut Vec<AudioCommand>,
        read_dialogue_ips: &'a mut BTreeSet<u32>,
        route_visited_ips: &'a mut BTreeSet<u32>,
        pending_transition: &'a mut Option<SceneTransitionCompiled>,
    ) -> Self {
        Self {
            state,
            script_events,
            audio_commands,
            read_dialogue_ips,
            route_visited_ips,
            pending_transition,
        }
    }

    pub fn begin_event(&mut self) {
        let current_ip = self.state.position;
        self.route_visited_ips.insert(current_ip);
        *self.pending_transition = None;
    }

    pub fn advance_position(&mut self) -> VnResult<()> {
        let next = self.state.position.saturating_add(1);
        if next as usize >= self.script_events.len() {
            self.state.position = self.script_events.len() as u32;
            return Ok(());
        }
        self.state.position = next;
        self.route_visited_ips.insert(self.state.position);
        Ok(())
    }

    pub fn jump_to_ip(&mut self, target_ip: u32) -> VnResult<()> {
        if target_ip as usize > self.script_events.len() {
            return Err(VnError::InvalidScript(format!(
                "jump target '{target_ip}' outside script"
            )));
        }
        if target_ip as usize == self.script_events.len() {
            self.state.position = target_ip;
            return Ok(());
        }
        let scene = match self.script_events.get(target_ip as usize) {
            Some(EventCompiled::Scene(scene)) => Some(scene.clone()),
            _ => None,
        };
        self.state.position = target_ip;
        self.route_visited_ips.insert(target_ip);
        if let Some(scene) = scene {
            let before_music = self.state.visual.music.clone();
            self.state.visual.apply_scene(&scene);
            append_music_delta(before_music, &self.state.visual.music, self.audio_commands);
        }
        Ok(())
    }

    pub(super) fn current_ip(&self) -> u32 {
        self.state.position
    }

    pub(super) fn evaluate_cond(&self, cond: &CondCompiled) -> bool {
        match cond {
            CondCompiled::Flag { flag_id, is_set } => self.state.get_flag(*flag_id) == *is_set,
            CondCompiled::VarCmp { var_id, op, value } => {
                let var_val = self.state.get_var(*var_id);
                match op {
                    CmpOp::Eq => var_val == *value,
                    CmpOp::Ne => var_val != *value,
                    CmpOp::Lt => var_val < *value,
                    CmpOp::Le => var_val <= *value,
                    CmpOp::Gt => var_val > *value,
                    CmpOp::Ge => var_val >= *value,
                }
            }
        }
    }
}

pub struct PreviewCtx<'a> {
    pub(super) visual: &'a mut VisualState,
}

impl<'a> PreviewCtx<'a> {
    pub fn new(visual: &'a mut VisualState) -> Self {
        Self { visual }
    }

    pub fn visual(&self) -> &VisualState {
        self.visual
    }

    pub fn visual_mut(&mut self) -> &mut VisualState {
        self.visual
    }
}

pub struct SceneFrameCtx<'a> {
    pub(super) commands: &'a mut Vec<RenderCommand>,
    pub(super) interactions: &'a mut Vec<InteractionSpec>,
}

impl<'a> SceneFrameCtx<'a> {
    pub fn new(
        commands: &'a mut Vec<RenderCommand>,
        interactions: &'a mut Vec<InteractionSpec>,
    ) -> Self {
        Self {
            commands,
            interactions,
        }
    }
}

pub struct ValidationCtx<'a> {
    pub(super) graph: &'a NodeGraph,
    pub(super) node_id: u32,
    pub(super) script_labels: &'a BTreeSet<String>,
    pub(super) asset_exists: &'a dyn Fn(&str) -> bool,
}

impl<'a> ValidationCtx<'a> {
    pub fn new(
        graph: &'a NodeGraph,
        node_id: u32,
        script_labels: &'a BTreeSet<String>,
        asset_exists: &'a dyn Fn(&str) -> bool,
    ) -> Self {
        Self {
            graph,
            node_id,
            script_labels,
            asset_exists,
        }
    }

    pub fn graph(&self) -> &NodeGraph {
        self.graph
    }

    pub fn node_id(&self) -> u32 {
        self.node_id
    }

    pub fn script_labels(&self) -> &BTreeSet<String> {
        self.script_labels
    }

    pub fn asset_exists(&self, asset: &str) -> bool {
        (self.asset_exists)(asset)
    }
}

pub struct QuickFixCtx<'a> {
    pub(super) graph: &'a NodeGraph,
}

impl<'a> QuickFixCtx<'a> {
    pub fn new(graph: &'a NodeGraph) -> Self {
        Self { graph }
    }

    pub fn graph(&self) -> &NodeGraph {
        self.graph
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorQuickFixRisk {
    Safe,
    Review,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BehaviorQuickFix {
    pub fix_id: &'static str,
    pub title_es: &'static str,
    pub title_en: &'static str,
    pub risk: BehaviorQuickFixRisk,
    pub structural: bool,
}

impl BehaviorQuickFix {
    pub const fn new(
        fix_id: &'static str,
        title_es: &'static str,
        title_en: &'static str,
        risk: BehaviorQuickFixRisk,
        structural: bool,
    ) -> Self {
        Self {
            fix_id,
            title_es,
            title_en,
            risk,
            structural,
        }
    }
}

#[derive(Default)]
struct StringPool {
    cache: HashMap<String, SharedStr>,
}

impl StringPool {
    fn intern(&mut self, value: &str) -> SharedStr {
        if let Some(existing) = self.cache.get(value) {
            return existing.clone();
        }
        let shared: SharedStr = Arc::from(value);
        self.cache.insert(value.to_string(), shared.clone());
        shared
    }
}

fn get_or_insert_id(map: &mut HashMap<String, u32>, key: &str) -> VnResult<u32> {
    if let Some(id) = map.get(key) {
        return Ok(*id);
    }
    let next_id =
        u32::try_from(map.len()).map_err(|_| VnError::InvalidScript("too many ids".to_string()))?;
    map.insert(key.to_string(), next_id);
    Ok(next_id)
}
