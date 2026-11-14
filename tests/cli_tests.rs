use std::process::Command;

#[test]
fn test_cli_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_milo"))
        .arg("--help")
        .output()
        .expect("Failed to execute milo CLI");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Proof-to-Move compiler"));
    assert!(stdout.contains("generate"));
    assert!(stdout.contains("verify"));
}

#[test]
fn test_cli_verify_multiplier2() {
    let output = Command::new(env!("CARGO_BIN_EXE_milo"))
        .args([
            "verify",
            "--vk",
            "examples/circuits/multiplier2_vk.json",
            "--proof",
            "examples/circuits/multiplier2_proof.json",
            "--public",
            "examples/circuits/multiplier2_public.json",
        ])
        .output()
        .expect("Failed to run verify on multiplier2");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Arkworks Groth16 verification PASSED"));
}

#[test]
fn test_cli_verify_poly4() {
    let output = Command::new(env!("CARGO_BIN_EXE_milo"))
        .args([
            "verify",
            "--vk",
            "examples/circuits/poly4_vk.json",
            "--proof",
            "examples/circuits/poly4_proof.json",
            "--public",
            "examples/circuits/poly4_public.json",
        ])
        .output()
        .expect("Failed to run verify on poly4");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Arkworks Groth16 verification PASSED"));
}

#[test]
fn test_cli_generate_code() {
    let temp_dir = std::env::temp_dir().join("milo_test_gen");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_milo"))
        .args([
            "generate",
            "--vk",
            "examples/circuits/multiplier2_vk.json",
            "--proof",
            "examples/circuits/multiplier2_proof.json",
            "--public",
            "examples/circuits/multiplier2_public.json",
            "--module-name",
            "test_verifier",
            "--out-dir",
            temp_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to generate Move verifier");

    assert!(output.status.success());
    assert!(temp_dir.join("test_verifier.move").exists());
    assert!(temp_dir.join("call_args.json").exists());

    let move_content = std::fs::read_to_string(temp_dir.join("test_verifier.move")).unwrap();
    assert!(move_content.contains("module deployer::test_verifier"));
    assert!(move_content.contains("verify_groth16_prepared"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
