//! Exercise the real executable's role/config checks without starting a network.

use std::process::Command;

#[test]
fn startup_rejects_unsafe_heads_and_missing_signing_configuration() {
    for (sequencer, expected) in [
        (
            Some("0x1111111111111111111111111111111111111111"),
            "remove EEZ_SEQUENCER_ADDRESS",
        ),
        (None, "EEZ_STATEFUL_PROOF_SIGNER_ADDR is required"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_eez-prover-stateful"));
        command
            .env_clear()
            .current_dir(dir.path())
            .args([
                "node",
                "--chain",
                "dev",
                "--disable-discovery",
                "--log.file.max-files=0",
            ])
            .arg("--datadir")
            .arg(dir.path().join("db"))
            .arg("--log.file.directory")
            .arg(dir.path().join("logs"));
        if let Some(sequencer) = sequencer {
            command.env("EEZ_SEQUENCER_ADDRESS", sequencer);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?}, got {stderr}"
        );
    }
}
