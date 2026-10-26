use super::{PyEngine, StepResult};
use crate::bindings::types::vn_error_to_py;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use visual_novel_engine::runtime::{EventCompiled, ExternalCallOutcome};

impl PyEngine {
    pub(super) fn ensure_execution_available(&self) -> PyResult<()> {
        if self.dispatching_handler {
            return Err(PyRuntimeError::new_err(
                "engine execution cannot advance while an ext_call callback is active",
            ));
        }
        Ok(())
    }

    pub(super) fn dispatch_step(slf: Py<Self>, py: Python<'_>) -> PyResult<StepResult> {
        let (event, dispatch) = {
            let mut engine = slf.try_borrow_mut(py)?;
            engine.ensure_execution_available()?;
            let (audio, change) = engine.inner.step().map_err(vn_error_to_py)?;
            engine.last_audio_commands = audio;
            let event = change.event;
            let dispatch = if let EventCompiled::ExtCall { command, args } = &event {
                if !engine.allowed_ext_call_commands.contains(command.as_str()) {
                    let msg = format!("ext_call '{command}' denied by capability policy");
                    engine.last_ext_call_error = Some(msg.clone());
                    return Err(PyRuntimeError::new_err(msg));
                }
                let Some(handler) = engine.handler.as_ref().map(|handler| handler.clone_ref(py))
                else {
                    let msg = format!("ext_call '{command}' requires a registered handler");
                    engine.last_ext_call_error = Some(msg.clone());
                    return Err(PyRuntimeError::new_err(msg));
                };
                let ip = engine.inner.state().position;
                engine.dispatching_handler = true;
                Some((handler, ip, command.clone(), args.clone()))
            } else {
                engine.last_ext_call_error = None;
                None
            };
            (event, dispatch)
        };

        if let Some((handler, event_ip, command, args)) = dispatch {
            // Release PyRefMut before entering Python: callbacks may inspect the
            // engine and enqueue audio. Only execution-changing reentry is denied.
            let result = handler.call1(py, (command.as_str(), args));
            let mut engine = slf.try_borrow_mut(py)?;
            engine.dispatching_handler = false;
            if let Err(error) = result {
                let msg = format!("ExtCall handler error for '{command}': {error}");
                engine.last_ext_call_error = Some(msg.clone());
                let result = engine
                    .inner
                    .complete_external_call(ExternalCallOutcome::failed(
                        event_ip,
                        command,
                        msg.clone(),
                    ));
                let msg = result.err().map_or(msg, |error| error.to_string());
                return Err(PyRuntimeError::new_err(msg));
            }
            engine
                .inner
                .complete_external_call(ExternalCallOutcome::succeeded(event_ip, command))
                .map_err(vn_error_to_py)?;
            let callback_audio = engine.inner.take_audio_commands();
            engine.last_audio_commands.extend(callback_audio);
            engine.last_ext_call_error = None;
        }

        let engine = slf.try_borrow(py)?;
        Ok(StepResult {
            event: event.to_python_dict(py)?,
            audio: engine.get_last_audio_commands(py)?,
        })
    }
}
