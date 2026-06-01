use super::*;

pub(super) fn print_json_envelope(envelope: CliEnvelope) {
    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            println!(
                "{{\"ok\":false,\"code\":\"envelope_serialization_error\",\"data\":null,\"error\":\"{}\"}}",
                err
            );
        }
    }
}

pub(super) fn normalize_cli_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
