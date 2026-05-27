use crate::event::EventRaw;

pub(super) fn targets_end_label(event: &EventRaw) -> bool {
    match event {
        EventRaw::Jump { target } | EventRaw::JumpIf { target, .. } => target == "__end",
        EventRaw::Choice(choice) => choice.options.iter().any(|option| option.target == "__end"),
        _ => false,
    }
}
