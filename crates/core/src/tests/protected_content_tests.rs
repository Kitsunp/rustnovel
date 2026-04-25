use super::*;

#[test]
fn protected_content_roundtrips_without_plaintext_ciphertext() {
    let key = b"development-key-for-protected-builds";
    let salt = *b"0123456789abcdef";
    let plaintext = br#"{"text":"secret route","next":"ending_a"}"#;

    let chunk = protect_content(key, "script", salt, 7, plaintext);
    assert_ne!(chunk.ciphertext, plaintext);

    let opened = open_protected_content(key, &chunk).expect("authenticated content");
    assert_eq!(opened, plaintext);
}

#[test]
fn protected_content_detects_tampering() {
    let key = b"development-key-for-protected-builds";
    let salt = *b"abcdef0123456789";
    let mut chunk = protect_content(key, "graph", salt, 9, b"node links");
    chunk.ciphertext[0] ^= 0x55;

    let error = open_protected_content(key, &chunk).expect_err("tamper should fail");
    assert_eq!(error, ProtectedContentError::AuthenticationFailed);
}

#[test]
fn protected_content_domain_separates_streams() {
    let key = b"development-key-for-protected-builds";
    let salt = *b"fedcba9876543210";
    let plaintext = b"shared text";

    let script = protect_content(key, "script", salt, 1, plaintext);
    let graph = protect_content(key, "graph", salt, 1, plaintext);

    assert_ne!(script.ciphertext, graph.ciphertext);
}
