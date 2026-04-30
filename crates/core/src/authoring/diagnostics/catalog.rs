use super::super::LintCode;
use super::DiagnosticLanguage;

pub(super) struct CatalogEntry {
    pub title: &'static str,
    pub what_happened: &'static str,
    pub root_cause: &'static str,
    pub why_failed: &'static str,
    pub consequence: &'static str,
    pub how_to_fix: &'static str,
    pub action_steps: &'static [&'static str],
    pub expected: &'static str,
}

struct Spec {
    title: &'static str,
    what: &'static str,
    root: &'static str,
    why: &'static str,
    consequence: &'static str,
    fix: &'static str,
    steps: &'static [&'static str],
    expected: &'static str,
}

pub(super) fn message_key(code: LintCode) -> String {
    format!(
        "diagnostic.{}",
        code.label().to_ascii_lowercase().replace('_', ".")
    )
}

pub(super) fn docs_ref(code: LintCode) -> String {
    format!(
        "docs/diagnostics/authoring.md#{}",
        code.label().to_ascii_lowercase().replace('_', "-")
    )
}

pub(super) fn entry(code: LintCode, language: DiagnosticLanguage) -> CatalogEntry {
    let spec = match language {
        DiagnosticLanguage::Es => spec_es(code),
        DiagnosticLanguage::En => spec_en(code),
    };
    CatalogEntry {
        title: spec.title,
        what_happened: spec.what,
        root_cause: spec.root,
        why_failed: spec.why,
        consequence: spec.consequence,
        how_to_fix: spec.fix,
        action_steps: spec.steps,
        expected: spec.expected,
    }
}

#[rustfmt::skip]
fn spec_es(code: LintCode) -> Spec {
    let why = "El contrato core/editor/runtime necesita una regla determinista para reproducir, exportar y explicar el resultado.";
    let consequence = "Si no se corrige, el reporte puede ocultar divergencias entre preview, CLI, Python o runtime.";
    let steps = &["Abrir la ubicacion marcada por el diagnostico.", "Corregir el dato semantico y volver a validar."];
    let (title, what, root, fix, expected) = match code {
        LintCode::MissingStart => ("Falta nodo Start", "El grafo no tiene entrada.", "No existe un Start exportable.", "Agrega un Start y conectalo al primer evento real.", "Un Start conectado."),
        LintCode::MultipleStart => ("Multiples nodos Start", "Hay mas de una entrada.", "Varios Start compiten como punto inicial.", "Conserva un Start oficial y elimina o reconecta los demas.", "Un unico Start."),
        LintCode::UnreachableNode => ("Nodo inalcanzable", "Un nodo no se visita desde Start.", "No hay ruta de conexiones hacia ese nodo.", "Conecta el nodo a una rama valida o mantenlo como borrador no exportable.", "Nodos exportables alcanzables."),
        LintCode::PotentialLoop => ("Bucle potencial", "El flujo puede volver a un nodo visitado.", "Hay un ciclo alcanzable.", "Confirma que el ciclo sea intencional y agrega salida o condicion.", "Ciclo con salida auditable."),
        LintCode::DeadEnd => ("Nodo sin salida", "Una ruta queda sin continuacion.", "El nodo no tiene salida hacia otro evento o End.", "Conectalo al siguiente evento o a End.", "Ruta con terminal claro."),
        LintCode::ChoiceNoOptions => ("Choice sin opciones", "Una eleccion no ofrece rutas.", "La lista de opciones esta vacia.", "Agrega opciones reales o un placeholder de revision.", "Choice con opciones."),
        LintCode::ChoiceOptionUnlinked => ("Opcion sin conexion", "Una opcion no tiene destino.", "El puerto/target de la opcion no resuelve.", "Conecta la opcion al nodo destino o elimina la opcion.", "Cada opcion con destino."),
        LintCode::ChoicePortOutOfRange => ("Puerto Choice fuera de rango", "Una conexion usa puerto inexistente.", "El indice excede las opciones del Choice.", "Elimina la conexion invalida y reconecta desde un puerto existente.", "Puertos dentro de rango."),
        LintCode::AudioAssetMissing => ("Audio faltante", "Una accion play no tiene asset valido.", "La ruta de audio esta ausente o no existe.", "Elige un archivo de audio valido o cambia la accion a stop.", "Play con asset existente."),
        LintCode::AudioAssetEmpty => ("Audio vacio", "Un campo de audio esta vacio.", "El valor contiene espacios o cadena vacia.", "Limpia el campo o selecciona un asset real.", "Audio None o ruta valida."),
        LintCode::AssetReferenceMissing => ("Asset inexistente", "Una referencia no se encuentra.", "El asset no existe bajo la raiz del proyecto.", "Importa el asset o corrige la ruta relativa.", "Asset existente."),
        LintCode::SceneBackgroundEmpty => ("Background vacio", "La escena declara fondo sin ruta.", "El campo background esta vacio.", "Limpia el valor o selecciona una imagen de fondo.", "Background None o imagen valida."),
        LintCode::UnsafeAssetPath => ("Ruta insegura", "Una ruta escapa del proyecto o es absoluta.", "La referencia viola la politica segura de paths.", "Reimporta el archivo para copiarlo dentro de assets.", "Ruta relativa segura."),
        LintCode::InvalidAudioChannel => ("Canal de audio invalido", "El canal no existe en runtime.", "El nombre no pertenece al contrato de audio.", "Usa bgm, sfx o voice segun corresponda.", "Canal soportado."),
        LintCode::InvalidAudioAction => ("Accion de audio invalida", "La accion no esta soportada.", "El valor no mapea a play/stop u operacion valida.", "Selecciona una accion soportada.", "Accion soportada."),
        LintCode::InvalidAudioVolume => ("Volumen invalido", "El volumen sale del rango.", "El valor no es finito o no esta entre 0 y 1.", "Ajusta el volumen a un numero finito entre 0 y 1.", "Volumen 0..1."),
        LintCode::InvalidAudioFade => ("Fade invalido", "La duracion de fade no es valida.", "El valor es negativo, infinito o fuera de limite.", "Usa una duracion finita no negativa.", "Fade finito."),
        LintCode::InvalidCharacterScale => ("Escala de personaje invalida", "Un personaje tiene escala imposible.", "La escala no es finita o visible.", "Restablece la escala a un rango positivo visible.", "Escala finita positiva."),
        LintCode::InvalidTransitionDuration => ("Duracion de transicion invalida", "La transicion tiene tiempo invalido.", "La duracion no es finita o es negativa.", "Usa una duracion finita no negativa.", "Duracion valida."),
        LintCode::InvalidTransitionKind => ("Tipo de transicion invalido", "La transicion usa tipo desconocido.", "El renderer no reconoce el efecto.", "Cambia a fade, dissolve u otro tipo soportado.", "Tipo soportado."),
        LintCode::EmptyCharacterName => ("Nombre de personaje vacio", "Una entidad visual no tiene nombre.", "La escena no puede correlacionar pose, binding y speaker.", "Asigna un nombre estable o elimina la entidad vacia.", "Personaje con nombre."),
        LintCode::EmptySpeakerName => ("Speaker vacio", "Un dialogo no indica hablante.", "El texto pierde trazabilidad con personaje/narrador.", "Asigna narrador o personaje hablante.", "Dialogo con speaker."),
        LintCode::EmptyJumpTarget => ("Jump sin destino", "Un salto no tiene label.", "El target textual esta vacio.", "Selecciona un destino existente o conecta el puerto.", "Jump con target."),
        LintCode::MissingJumpTarget => ("Destino Jump inexistente", "Un salto apunta a label ausente.", "El target roto se preservo para no ocultar perdida.", "Crea el label/nodo destino o corrige el target.", "Target existente."),
        LintCode::EmptyStateKey => ("Llave de estado vacia", "Una condicion o mutacion no tiene key.", "JumpIf/SetFlag/SetVariable no puede leer o escribir estado.", "Asigna una key estable no vacia.", "Key no vacia."),
        LintCode::InvalidLayoutPosition => ("Posicion visual invalida", "Un nodo tiene coordenadas corruptas.", "La posicion contiene NaN, infinito o valores extremos.", "Ejecuta reset/auto layout o mueve el nodo a coordenadas normales.", "Coordenadas finitas."),
        LintCode::PlaceholderChoiceOption => ("Opcion placeholder", "Una opcion generada no fue editada.", "El texto sigue siendo marcador temporal.", "Reemplaza el placeholder por texto final.", "Texto final."),
        LintCode::ContractUnsupportedExport => ("Export no soportado", "Un nodo no puede exportarse fielmente.", "El contrato runtime/editor no cubre esa semantica.", "Convierte el nodo a una primitiva soportada o documenta capability.", "Nodo exportable."),
        LintCode::GenericEventUnchecked => ("Generic sin verificar", "Un evento generico requiere revision.", "El editor conserva payload que no interpreta por completo.", "Resuelve a un nodo tipado o acepta explicitamente el Generic.", "Evento revisado."),
        LintCode::CompileError => ("Error de compilacion", "ScriptRaw no compila.", "La conversion a runtime genero contrato invalido.", "Corrige errores previos y vuelve a compilar.", "Script compilable."),
        LintCode::RuntimeInitError => ("Error runtime init", "El engine no pudo iniciar.", "Seguridad o limites rechazaron el runtime compilado.", "Revisa politicas, labels y recursos antes de ejecutar.", "Engine inicializado."),
        LintCode::DryRunUnreachableCompiled => ("Evento compilado inalcanzable", "Un event_ip compilado no se visita.", "StoryGraph no encuentra entrada hacia ese evento.", "Reconecta o elimina el evento inalcanzable.", "Eventos alcanzables."),
        LintCode::DryRunStepLimit => ("Dry-run llego al limite", "La simulacion fue truncada.", "Hay ciclo o flujo largo sin cierre observado.", "Agrega salida al ciclo o sube limites conscientemente.", "Dry-run sin truncar."),
        LintCode::DryRunRuntimeError => ("Error en dry-run", "La simulacion fallo al ejecutar.", "Un evento compilado fallo al avanzar.", "Usa el repro generado y corrige el event_ip marcado.", "Dry-run sin errores."),
        LintCode::DryRunParityMismatch => ("Diferencia preview/runtime", "Las firmas no coinciden.", "Editor y engine interpretan distinto la misma semantica.", "Migra la semantica al core o corrige el nodo.", "Firmas equivalentes."),
        LintCode::DryRunExtCallSimulated => ("ExtCall simulado", "Se encontro una llamada externa.", "La validacion headless no ejecuta plugins por seguridad.", "Documenta la capability o reemplaza por evento nativo.", "Capability declarada."),
        LintCode::DryRunFinished => ("Dry-run terminado", "La ruta probada termino limpio.", "La simulacion alcanzo EndOfScript.", "Revisa cobertura de rutas para ramas alternativas.", "Fin limpio."),
    };
    Spec { title, what, root, why, consequence, fix, steps, expected }
}

#[rustfmt::skip]
fn spec_en(code: LintCode) -> Spec {
    let why = "The core/editor/runtime contract needs a deterministic rule to reproduce, export and explain the result.";
    let consequence = "If left unresolved, reports can hide divergence between preview, CLI, Python or runtime.";
    let steps = &["Open the location marked by the diagnostic.", "Fix the semantic data and validate again."];
    let (title, what, root, fix, expected) = match code {
        LintCode::MissingStart => ("Missing Start node", "The graph has no entry.", "No exportable Start exists.", "Add one Start and connect it to the first real event.", "One connected Start."),
        LintCode::MultipleStart => ("Multiple Start nodes", "More than one entry exists.", "Several Start nodes compete as initial point.", "Keep one official Start and remove or reconnect the rest.", "Exactly one Start."),
        LintCode::UnreachableNode => ("Unreachable node", "A node cannot be visited from Start.", "No connection path reaches that node.", "Connect it to a valid branch or keep it as non-exported draft.", "Reachable exportable nodes."),
        LintCode::PotentialLoop => ("Potential loop", "Flow can return to a visited node.", "A reachable cycle exists.", "Confirm intent and add an exit or condition.", "Cycle with auditable exit."),
        LintCode::DeadEnd => ("Dead-end node", "A route has no continuation.", "The node has no outgoing event or End.", "Connect it to the next event or End.", "Clear terminal route."),
        LintCode::ChoiceNoOptions => ("Choice without options", "A choice offers no routes.", "The option list is empty.", "Add real options or a review placeholder.", "Choice with options."),
        LintCode::ChoiceOptionUnlinked => ("Unlinked Choice option", "An option has no destination.", "The option port/target does not resolve.", "Connect it to a destination or remove it.", "Every option has a target."),
        LintCode::ChoicePortOutOfRange => ("Choice port out of range", "A connection uses a nonexistent port.", "The index exceeds available options.", "Delete the invalid edge and reconnect from an existing port.", "Ports within range."),
        LintCode::AudioAssetMissing => ("Missing audio asset", "A play action has no valid asset.", "The audio path is absent or missing.", "Choose a valid audio file or change the action to stop.", "Play with existing asset."),
        LintCode::AudioAssetEmpty => ("Empty audio asset", "An audio field is empty.", "The value is whitespace or empty.", "Clear the field or choose a real asset.", "Audio None or valid path."),
        LintCode::AssetReferenceMissing => ("Missing asset reference", "A reference cannot be found.", "The asset does not exist under project root.", "Import the asset or correct the relative path.", "Existing asset."),
        LintCode::SceneBackgroundEmpty => ("Empty scene background", "A scene declares a background with no path.", "The background field is empty.", "Clear it or choose a background image.", "None or valid image."),
        LintCode::UnsafeAssetPath => ("Unsafe asset path", "A path escapes the project or is absolute.", "The reference violates safe path policy.", "Reimport the file so it is copied into assets.", "Safe relative path."),
        LintCode::InvalidAudioChannel => ("Invalid audio channel", "The channel does not exist in runtime.", "The name is outside the audio contract.", "Use bgm, sfx or voice as appropriate.", "Supported channel."),
        LintCode::InvalidAudioAction => ("Invalid audio action", "The action is unsupported.", "The value does not map to play/stop or a valid operation.", "Choose a supported action.", "Supported action."),
        LintCode::InvalidAudioVolume => ("Invalid audio volume", "Volume is out of range.", "The value is not finite or not within 0..1.", "Set a finite value between 0 and 1.", "Volume 0..1."),
        LintCode::InvalidAudioFade => ("Invalid audio fade", "Fade duration is invalid.", "The value is negative, infinite or out of limit.", "Use a finite non-negative duration.", "Finite fade."),
        LintCode::InvalidCharacterScale => ("Invalid character scale", "A character has impossible scale.", "Scale is not finite or visible.", "Reset scale to a positive visible range.", "Finite positive scale."),
        LintCode::InvalidTransitionDuration => ("Invalid transition duration", "Transition time is invalid.", "Duration is not finite or is negative.", "Use a finite non-negative duration.", "Valid duration."),
        LintCode::InvalidTransitionKind => ("Invalid transition kind", "Transition kind is unknown.", "The renderer does not know the effect.", "Change to fade, dissolve or another supported kind.", "Supported kind."),
        LintCode::EmptyCharacterName => ("Empty character name", "A visual entity has no name.", "The scene cannot correlate pose, binding and speaker.", "Assign a stable name or remove the empty entity.", "Named character."),
        LintCode::EmptySpeakerName => ("Empty speaker", "A dialogue has no speaker.", "Text loses traceability to character/narrator.", "Assign a narrator or speaking character.", "Dialogue with speaker."),
        LintCode::EmptyJumpTarget => ("Empty jump target", "A jump has no label.", "The textual target is empty.", "Select an existing destination or connect the port.", "Jump with target."),
        LintCode::MissingJumpTarget => ("Missing jump target", "A jump points to an absent label.", "The broken target was preserved to avoid hiding data loss.", "Create the label/node or correct the target.", "Existing target."),
        LintCode::EmptyStateKey => ("Empty state key", "A condition or mutation has no key.", "JumpIf/SetFlag/SetVariable cannot read or write state.", "Assign a stable non-empty key.", "Non-empty key."),
        LintCode::InvalidLayoutPosition => ("Invalid visual position", "A node has corrupted coordinates.", "Position contains NaN, infinity or extreme values.", "Run reset/auto layout or move the node to normal coordinates.", "Finite coordinates."),
        LintCode::PlaceholderChoiceOption => ("Placeholder option", "A generated option was not edited.", "Text still contains a temporary marker.", "Replace the placeholder with final text.", "Final text."),
        LintCode::ContractUnsupportedExport => ("Unsupported export contract", "A node cannot export faithfully.", "Runtime/editor contract does not cover this semantic.", "Convert to a supported primitive or document capability.", "Exportable node."),
        LintCode::GenericEventUnchecked => ("Unchecked Generic event", "A generic event needs review.", "The editor preserves payload it cannot fully interpret.", "Resolve to a typed node or explicitly accept the Generic.", "Reviewed event."),
        LintCode::CompileError => ("Compile error", "ScriptRaw does not compile.", "Runtime conversion produced an invalid contract.", "Fix earlier errors and compile again.", "Compilable script."),
        LintCode::RuntimeInitError => ("Runtime init error", "Engine could not start.", "Security or limits rejected the compiled runtime.", "Review policies, labels and resources before running.", "Initialized engine."),
        LintCode::DryRunUnreachableCompiled => ("Unreachable compiled event", "A compiled event_ip is not visited.", "StoryGraph finds no incoming route to that event.", "Reconnect or remove the unreachable event.", "Reachable events."),
        LintCode::DryRunStepLimit => ("Dry-run reached limit", "Simulation was truncated.", "A cycle or long flow has no observed closure.", "Add an exit or deliberately raise QA limits.", "Untruncated dry-run."),
        LintCode::DryRunRuntimeError => ("Dry-run runtime error", "Simulation failed while executing.", "A compiled event failed when advanced.", "Use the generated repro and fix the marked event_ip.", "Dry-run without errors."),
        LintCode::DryRunParityMismatch => ("Preview/runtime mismatch", "Signatures do not match.", "Editor and engine interpret the same semantic differently.", "Move the semantic to core or fix the node.", "Equivalent signatures."),
        LintCode::DryRunExtCallSimulated => ("ExtCall simulated", "An external call was found.", "Headless validation does not execute plugins for safety.", "Document the capability or replace it with a native event.", "Declared capability."),
        LintCode::DryRunFinished => ("Dry-run finished", "The tested route ended cleanly.", "Simulation reached EndOfScript.", "Review route coverage for alternate branches.", "Clean finish."),
    };
    Spec { title, what, root, why, consequence, fix, steps, expected }
}
