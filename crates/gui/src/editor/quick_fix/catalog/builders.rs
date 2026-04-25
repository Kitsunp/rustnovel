use super::super::{QuickFixCandidate, QuickFixRisk};

struct CandidateSpec {
    fix_id: &'static str,
    title_es: &'static str,
    title_en: &'static str,
    preconditions_es: &'static str,
    preconditions_en: &'static str,
    postconditions_es: &'static str,
    postconditions_en: &'static str,
    risk: QuickFixRisk,
    structural: bool,
}

fn candidate(spec: CandidateSpec) -> QuickFixCandidate {
    QuickFixCandidate {
        fix_id: spec.fix_id,
        title_es: spec.title_es,
        title_en: spec.title_en,
        preconditions_es: spec.preconditions_es,
        preconditions_en: spec.preconditions_en,
        postconditions_es: spec.postconditions_es,
        postconditions_en: spec.postconditions_en,
        risk: spec.risk,
        structural: spec.structural,
    }
}

pub(crate) fn fix_choice_add_default_option() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "choice_add_default_option",
        title_es: "Agregar opcion por defecto",
        title_en: "Add default option",
        preconditions_es: "Nodo Choice sin opciones.",
        preconditions_en: "Choice node has no options.",
        postconditions_es: "Choice queda con al menos una opcion.",
        postconditions_en: "Choice has at least one option.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_choice_link_unlinked_to_end() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "choice_link_unlinked_to_end",
        title_es: "Conectar opciones sueltas a End",
        title_en: "Connect dangling options to End",
        preconditions_es: "Choice con opciones sin conexion saliente.",
        preconditions_en: "Choice has options without outgoing links.",
        postconditions_es: "Cada opcion sin destino queda conectada a End.",
        postconditions_en: "Each unlinked option is connected to End.",
        risk: QuickFixRisk::Review,
        structural: true,
    })
}

pub(crate) fn fix_choice_expand_options_to_ports() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "choice_expand_options_to_ports",
        title_es: "Sincronizar opciones con puertos",
        title_en: "Sync options with connected ports",
        preconditions_es: "Hay conexiones de puertos fuera del rango de opciones.",
        preconditions_en: "There are connected ports beyond current option count.",
        postconditions_es: "Cantidad de opciones cubre todos los puertos conectados.",
        postconditions_en: "Option count covers all connected ports.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_add_missing_start() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "graph_add_start",
        title_es: "Agregar nodo Start",
        title_en: "Add Start node",
        preconditions_es: "No existe Start en el grafo.",
        preconditions_en: "No Start node exists in graph.",
        postconditions_es: "Grafo contiene Start y punto de entrada.",
        postconditions_en: "Graph contains Start entry point.",
        risk: QuickFixRisk::Review,
        structural: true,
    })
}

pub(crate) fn fix_dead_end_to_end() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "node_connect_dead_end_to_end",
        title_es: "Conectar nodo sin salida a End",
        title_en: "Connect dead-end node to End",
        preconditions_es: "Nodo con dead-end y sin salida.",
        preconditions_en: "Node has dead-end and no outgoing edge.",
        postconditions_es: "Nodo queda conectado a End.",
        postconditions_en: "Node gets an outgoing edge to End.",
        risk: QuickFixRisk::Review,
        structural: true,
    })
}

pub(crate) fn fix_fill_speaker() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "dialogue_fill_speaker",
        title_es: "Asignar speaker Narrator",
        title_en: "Set speaker to Narrator",
        preconditions_es: "Dialogue con speaker vacio.",
        preconditions_en: "Dialogue has empty speaker.",
        postconditions_es: "Speaker no vacio.",
        postconditions_en: "Speaker is non-empty.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_fill_jump_target() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "jump_set_start_target",
        title_es: "Asignar target start",
        title_en: "Set target to start",
        preconditions_es: "Jump/JumpIf con target vacio.",
        preconditions_en: "Jump/JumpIf has empty target.",
        postconditions_es: "Target apuntando a start.",
        postconditions_en: "Target points to start.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_transition_kind() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "transition_set_fade",
        title_es: "Normalizar tipo de transicion a fade",
        title_en: "Normalize transition kind to fade",
        preconditions_es: "Tipo de transicion fuera de contrato.",
        preconditions_en: "Transition kind outside contract.",
        postconditions_es: "Tipo valido (fade).",
        postconditions_en: "Valid kind (fade).",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_transition_duration() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "transition_set_default_duration",
        title_es: "Asignar duracion por defecto (300ms)",
        title_en: "Set default duration (300ms)",
        preconditions_es: "Duracion <= 0.",
        preconditions_en: "Duration <= 0.",
        postconditions_es: "Duracion valida > 0.",
        postconditions_en: "Valid duration > 0.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_audio_channel() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "audio_normalize_channel",
        title_es: "Normalizar canal de audio",
        title_en: "Normalize audio channel",
        preconditions_es: "Canal fuera de contrato.",
        preconditions_en: "Channel outside contract.",
        postconditions_es: "Canal valido (bgm/sfx/voice).",
        postconditions_en: "Valid channel (bgm/sfx/voice).",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_audio_action() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "audio_normalize_action",
        title_es: "Normalizar accion de audio",
        title_en: "Normalize audio action",
        preconditions_es: "Accion fuera de contrato.",
        preconditions_en: "Action outside contract.",
        postconditions_es: "Accion valida (play/stop/fade_out).",
        postconditions_en: "Valid action (play/stop/fade_out).",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_audio_volume() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "audio_clamp_volume",
        title_es: "Ajustar volumen al rango [0,1]",
        title_en: "Clamp volume to [0,1]",
        preconditions_es: "Volumen invalido o no finito.",
        preconditions_en: "Volume invalid or non-finite.",
        postconditions_es: "Volumen valido en [0,1].",
        postconditions_en: "Volume valid in [0,1].",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_audio_fade() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "audio_set_default_fade",
        title_es: "Asignar fade por defecto (250ms)",
        title_en: "Set default fade (250ms)",
        preconditions_es: "Accion stop/fade_out con fade invalido.",
        preconditions_en: "Stop/fade_out action has invalid fade.",
        postconditions_es: "Fade valido para stop/fade_out.",
        postconditions_en: "Valid fade for stop/fade_out.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_scene_bg_empty() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "scene_clear_empty_background",
        title_es: "Limpiar background vacio",
        title_en: "Clear empty background",
        preconditions_es: "Background declarado pero vacio.",
        preconditions_en: "Background declared but empty.",
        postconditions_es: "Background en None o valor valido.",
        postconditions_en: "Background is None or valid.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_audio_asset_empty() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "audio_clear_empty_asset",
        title_es: "Limpiar asset de audio vacio",
        title_en: "Clear empty audio asset",
        preconditions_es: "Asset de audio es cadena vacia.",
        preconditions_en: "Audio asset is an empty string.",
        postconditions_es: "Asset queda None para evitar ruta invalida.",
        postconditions_en: "Asset becomes None to avoid invalid path.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_scene_music_empty() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "scene_clear_empty_music",
        title_es: "Limpiar musica vacia en Scene",
        title_en: "Clear empty music in Scene",
        preconditions_es: "Scene con musica declarada pero vacia.",
        preconditions_en: "Scene has declared music path but it is empty.",
        postconditions_es: "Scene.music queda en None.",
        postconditions_en: "Scene.music becomes None.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}

pub(crate) fn fix_audio_missing_asset() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "audio_missing_asset_to_stop",
        title_es: "Normalizar play sin asset a stop",
        title_en: "Normalize play without asset to stop",
        preconditions_es: "AudioAction en play sin asset valido.",
        preconditions_en: "AudioAction is play without a valid asset.",
        postconditions_es: "Accion queda en stop con asset None.",
        postconditions_en: "Action is set to stop with asset None.",
        risk: QuickFixRisk::Review,
        structural: false,
    })
}

pub(crate) fn fix_clear_missing_asset_reference() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "clear_missing_asset_reference",
        title_es: "Limpiar referencia de asset inexistente",
        title_en: "Clear missing asset reference",
        preconditions_es: "Referencia de asset no existe en disco y campo es opcional.",
        preconditions_en: "Asset reference is missing on disk and field is optional.",
        postconditions_es: "Referencia se limpia para evitar fallo de carga.",
        postconditions_en: "Reference is cleared to avoid loading failure.",
        risk: QuickFixRisk::Review,
        structural: false,
    })
}

pub(crate) fn fix_clear_unsafe_asset_reference() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "clear_unsafe_asset_reference",
        title_es: "Limpiar referencia de asset insegura",
        title_en: "Clear unsafe asset reference",
        preconditions_es: "Referencia de asset viola politicas de ruta segura.",
        preconditions_en: "Asset reference violates safe-path policy.",
        postconditions_es: "Referencia insegura eliminada del nodo.",
        postconditions_en: "Unsafe reference is removed from node.",
        risk: QuickFixRisk::Review,
        structural: false,
    })
}

pub(crate) fn fix_character_entries() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "character_prune_or_fill_invalid_names",
        title_es: "Corregir nombres de personajes invalidos",
        title_en: "Fix invalid character names",
        preconditions_es: "Hay entradas de personaje con nombre vacio.",
        preconditions_en: "Character entries with empty names exist.",
        postconditions_es: "Entradas invalidas eliminadas o nombre por defecto aplicado.",
        postconditions_en: "Invalid entries pruned or default name applied.",
        risk: QuickFixRisk::Review,
        structural: false,
    })
}

pub(crate) fn fix_character_scale() -> QuickFixCandidate {
    candidate(CandidateSpec {
        fix_id: "character_set_default_scale",
        title_es: "Asignar escala por defecto (1.0)",
        title_en: "Set default scale (1.0)",
        preconditions_es: "Escala invalida o no finita.",
        preconditions_en: "Scale invalid or non-finite.",
        postconditions_es: "Escala valida > 0.",
        postconditions_en: "Scale valid > 0.",
        risk: QuickFixRisk::Safe,
        structural: false,
    })
}
