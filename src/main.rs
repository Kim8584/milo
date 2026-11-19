use std::fs::File;
use std::io::Read;
use std::path::Path;
use anyhow::{Context, Result};
use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine};
use ark_ec::pairing::Pairing;
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{Field, PrimeField};
use num_traits::Zero;
use ark_serialize::{CanonicalSerialize, Compress};
use clap::{Parser, Subcommand};
use num_bigint::BigUint;
use num_traits::Num;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "milo")]
#[command(about = "Milo: Proof-to-Move compiler for Aptos Groth16 (BN254) verifiers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a standalone Aptos Move verifier module and call arguments from snarkjs artifacts
    Generate {
        #[arg(long, help = "Path to snarkjs verification_key.json")]
        vk: String,
        #[arg(long, help = "Path to snarkjs proof.json")]
        proof: String,
        #[arg(long, help = "Path to snarkjs public.json")]
        public: String,
        #[arg(long, default_value = "groth16_verifier", help = "Move module name")]
        module_name: String,
        #[arg(long, default_value = ".", help = "Output directory for Move module and call_args")]
        out_dir: String,
    },
    /// Verify proof locally in Rust using Arkworks BN254 to ensure math correctness
    Verify {
        #[arg(long, help = "Path to snarkjs verification_key.json")]
        vk: String,
        #[arg(long, help = "Path to snarkjs proof.json")]
        proof: String,
        #[arg(long, help = "Path to snarkjs public.json")]
        public: String,
    },
    /// Print summary information and sizes for a verification key and proof
    Info {
        #[arg(long, help = "Path to snarkjs verification_key.json")]
        vk: String,
        #[arg(long, help = "Path to snarkjs proof.json")]
        proof: String,
        #[arg(long, help = "Path to snarkjs public.json")]
        public: String,
    },
}

#[derive(Deserialize, Serialize, Debug)]
struct SnarkJsGroth16VerificationKey {
    protocol: String,
    curve: String,
    vk_alpha_1: [String; 3],
    vk_beta_2: [[String; 2]; 3],
    vk_gamma_2: [[String; 2]; 3],
    vk_delta_2: [[String; 2]; 3],
    #[serde(rename = "IC")]
    ic: Vec<[String; 3]>,
}

#[derive(Deserialize, Serialize, Debug)]
struct SnarkJsGroth16Proof {
    pi_a: [String; 3],
    pi_b: [[String; 2]; 3],
    pi_c: [String; 3],
    protocol: String,
    curve: String,
}

type SnarkJsGroth16PublicInput = Vec<String>;

#[derive(Serialize, Deserialize, Debug)]
pub struct MoveCallArgs {
    pub proof_a_hex: String,
    pub proof_b_hex: String,
    pub proof_c_hex: String,
    pub public_inputs_hex: Vec<String>,
}

fn as_fq(repr: &str) -> Result<Fq> {
    let bytes = BigUint::from_str_radix(repr, 10).context("Invalid base 10 integer for Fq")?;
    Ok(Fq::from_be_bytes_mod_order(&bytes.to_bytes_be()))
}

fn as_fr(repr: &str) -> Result<Fr> {
    let bytes = BigUint::from_str_radix(repr, 10).context("Invalid base 10 integer for Fr")?;
    Ok(Fr::from_be_bytes_mod_order(&bytes.to_bytes_be()))
}

fn as_fq2(repr: &[String; 2]) -> Result<Fq2> {
    let c0 = as_fq(&repr[0])?;
    let c1 = as_fq(&repr[1])?;
    Ok(Fq2::new(c0, c1))
}

fn as_g1_affine(repr: &[String; 3]) -> Result<G1Affine> {
    let x = as_fq(&repr[0])?;
    let y = as_fq(&repr[1])?;
    let z = as_fq(&repr[2])?;
    if z == Fq::from(0u64) {
        Ok(G1Affine::zero())
    } else {
        let z_inv = z.inverse().context("G1 coordinate z not invertible")?;
        Ok(G1Affine::new(x * z_inv, y * z_inv))
    }
}

fn as_g2_affine(repr: &[[String; 2]; 3]) -> Result<G2Affine> {
    let x = as_fq2(&repr[0])?;
    let y = as_fq2(&repr[1])?;
    let z = as_fq2(&repr[2])?;
    if z == Fq2::from(0u64) {
        Ok(G2Affine::zero())
    } else {
        let z_inv = z.inverse().context("G2 coordinate z not invertible")?;
        Ok(G2Affine::new(x * z_inv, y * z_inv))
    }
}

fn to_hex_compressed<T: CanonicalSerialize>(item: &T) -> Result<String> {
    let mut buf = Vec::new();
    item.serialize_with_mode(&mut buf, Compress::Yes)
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    Ok(hex::encode(buf))
}

fn to_hex_uncompressed<T: CanonicalSerialize>(item: &T) -> Result<String> {
    let mut buf = Vec::new();
    item.serialize_with_mode(&mut buf, Compress::No)
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    Ok(hex::encode(buf))
}

fn read_file(path: &str) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("Failed to open file {}", path))?;
    let mut s = String::new();
    file.read_to_string(&mut s)?;
    Ok(s)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Verify { vk, proof, public } => {
            let vk_str = read_file(&vk)?;
            let vk_json: SnarkJsGroth16VerificationKey = serde_json::from_str(&vk_str)?;

            let proof_str = read_file(&proof)?;
            let proof_json: SnarkJsGroth16Proof = serde_json::from_str(&proof_str)?;

            let public_str = read_file(&public)?;
            let public_json: SnarkJsGroth16PublicInput = serde_json::from_str(&public_str)?;

            let alpha = as_g1_affine(&vk_json.vk_alpha_1)?;
            let beta = as_g2_affine(&vk_json.vk_beta_2)?;
            let gamma = as_g2_affine(&vk_json.vk_gamma_2)?;
            let delta = as_g2_affine(&vk_json.vk_delta_2)?;

            let a = as_g1_affine(&proof_json.pi_a)?;
            let b = as_g2_affine(&proof_json.pi_b)?;
            let c = as_g1_affine(&proof_json.pi_c)?;

            let mut ic_points = Vec::new();
            for pt in &vk_json.ic {
                ic_points.push(as_g1_affine(pt)?);
            }

            let mut pub_scalars = vec![Fr::from(1u64)];
            for p in &public_json {
                pub_scalars.push(as_fr(p)?);
            }

            assert_eq!(ic_points.len(), pub_scalars.len(), "Public inputs count mismatch with IC length");

            // Compute MSM: L = sum(pub_scalars[i] * ic[i])
            let mut l_proj = G1Projective::zero();
            for (pt, s) in ic_points.iter().zip(pub_scalars.iter()) {
                l_proj += pt.mul_bigint(s.into_bigint());
            }
            let l = l_proj.into_affine();

            // Check standard pairing: e(A, B) == e(alpha, beta) * e(L, gamma) * e(C, delta)
            let pairing_left = Bn254::pairing(a, b);
            let p_ab = Bn254::pairing(alpha, beta);
            let p_lg = Bn254::pairing(l, gamma);
            let p_cd = Bn254::pairing(c, delta);
            let pairing_right = p_ab + p_lg + p_cd;

            if pairing_left == pairing_right {
                println!("Arkworks Groth16 verification PASSED!");
            } else {
                eprintln!("Arkworks Groth16 verification FAILED!");
                std::process::exit(1);
            }
        }

        Commands::Info { vk, proof, public } => {
            let vk_str = read_file(&vk)?;
            let vk_json: SnarkJsGroth16VerificationKey = serde_json::from_str(&vk_str)?;

            let proof_str = read_file(&proof)?;
            let _proof_json: SnarkJsGroth16Proof = serde_json::from_str(&proof_str)?;

            let public_str = read_file(&public)?;
            let public_json: SnarkJsGroth16PublicInput = serde_json::from_str(&public_str)?;

            let num_pub = public_json.len();
            let num_ic = vk_json.ic.len();

            println!("=== Milo ZK Artifact Info ===");
            println!("Verification Key:");
            println!("  Curve: {}", vk_json.curve);
            println!("  Protocol: {}", vk_json.protocol);
            println!("  Public Inputs expected: {}", num_pub);
            println!("  IC elements: {}", num_ic);
            println!("Move Verifier Characteristics on Aptos:");
            println!("  Estimated prepared pairing gas: ~607.77 gas units");
            println!("  Estimated MSM gas: ~{:.2} gas units", 20.44 + (num_pub as f64 + 1.0) * 15.33);
            println!("  Estimated total verification cost: ~{:.2} gas units", 643.54 + (num_pub as f64) * 15.33);
        }

        Commands::Generate {
            vk,
            proof,
            public,
            module_name,
            out_dir,
        } => {
            let vk_str = read_file(&vk)?;
            let vk_json: SnarkJsGroth16VerificationKey = serde_json::from_str(&vk_str)?;

            let proof_str = read_file(&proof)?;
            let proof_json: SnarkJsGroth16Proof = serde_json::from_str(&proof_str)?;

            let public_str = read_file(&public)?;
            let public_json: SnarkJsGroth16PublicInput = serde_json::from_str(&public_str)?;

            let alpha = as_g1_affine(&vk_json.vk_alpha_1)?;
            let beta = as_g2_affine(&vk_json.vk_beta_2)?;
            let gamma = as_g2_affine(&vk_json.vk_gamma_2)?;
            let delta = as_g2_affine(&vk_json.vk_delta_2)?;

            let a = as_g1_affine(&proof_json.pi_a)?;
            let b = as_g2_affine(&proof_json.pi_b)?;
            let c = as_g1_affine(&proof_json.pi_c)?;

            // Precompute prepared parameters for Aptos
            let alpha_beta_pairing = Bn254::pairing(alpha, beta).0; // element of Fq12
            let gamma_neg = -gamma;
            let delta_neg = -delta;

            // Serialize points into Aptos formats
            let alpha_beta_fq12_hex = to_hex_uncompressed(&alpha_beta_pairing)?;
            let gamma_neg_hex = to_hex_compressed(&gamma_neg)?;
            let delta_neg_hex = to_hex_compressed(&delta_neg)?;

            let vk_alpha_g1_hex = to_hex_compressed(&alpha)?;
            let vk_beta_g2_hex = to_hex_compressed(&beta)?;
            let vk_gamma_g2_hex = to_hex_compressed(&gamma)?;
            let vk_delta_g2_hex = to_hex_compressed(&delta)?;

            let proof_a_hex = to_hex_compressed(&a)?;
            let proof_b_hex = to_hex_compressed(&b)?;
            let proof_c_hex = to_hex_compressed(&c)?;

            let mut ic_hex_list = Vec::new();
            for pt in &vk_json.ic {
                let g1 = as_g1_affine(pt)?;
                ic_hex_list.push(to_hex_compressed(&g1)?);
            }

            let mut pub_inputs_hex = Vec::new();
            for p in &public_json {
                let fr = as_fr(p)?;
                pub_inputs_hex.push(to_hex_uncompressed(&fr)?);
            }

            // Generate call_args.json
            let call_args = MoveCallArgs {
                proof_a_hex: proof_a_hex.clone(),
                proof_b_hex: proof_b_hex.clone(),
                proof_c_hex: proof_c_hex.clone(),
                public_inputs_hex: pub_inputs_hex.clone(),
            };

            let out_path = Path::new(&out_dir);
            std::fs::create_dir_all(out_path)?;

            let call_args_path = out_path.join("call_args.json");
            std::fs::write(&call_args_path, serde_json::to_string_pretty(&call_args)?)?;

            // Generate Move source code
            let ic_move_vector = ic_hex_list
                .iter()
                .map(|h| format!("x\"{}\"", h))
                .collect::<Vec<_>>()
                .join(", ");

            let pub_inputs_move_vector = pub_inputs_hex
                .iter()
                .map(|h| format!("x\"{}\"", h))
                .collect::<Vec<_>>()
                .join(", ");

            let move_code = format!(
r#"// Auto-generated Groth16 Verifier in Move for Aptos
// Generated by proof-to-Move toolkit
module deployer::{module_name} {{
    use std::vector;
    use std::option;
    use aptos_std::crypto_algebra::{{
        Element, deserialize, multi_scalar_mul, multi_pairing, eq, upcast, pairing, add, zero, from_u64
    }};
    use std::bn254_algebra::{{
        G1, G2, Gt, Fq12, Fr, FormatG1Compr, FormatG2Compr, FormatFq12LscLsb, FormatFrLsb
    }};

    /// Error codes
    const E_DESERIALIZATION_FAILED: u64 = 1001;
    const E_PROOF_VERIFICATION_FAILED: u64 = 1002;
    const E_INPUT_SIZE_MISMATCH: u64 = 1003;

    // --- Embedded Prepared Verification Key Constants ---
    const PVK_ALPHA_BETA_FQ12: vector<u8> = x"{alpha_beta_fq12_hex}";
    const PVK_GAMMA_G2_NEG: vector<u8> = x"{gamma_neg_hex}";
    const PVK_DELTA_G2_NEG: vector<u8> = x"{delta_neg_hex}";

    // --- Unprepared Verification Key Constants (for Naive verification) ---
    const VK_ALPHA_G1: vector<u8> = x"{vk_alpha_g1_hex}";
    const VK_BETA_G2: vector<u8> = x"{vk_beta_g2_hex}";
    const VK_GAMMA_G2: vector<u8> = x"{vk_gamma_g2_hex}";
    const VK_DELTA_G2: vector<u8> = x"{vk_delta_g2_hex}";

    /// Returns the vector of gamma_abc IC points in G1
    public fun get_vk_gamma_abc(): vector<Element<G1>> {{
        let raw_ic: vector<vector<u8>> = vector[{ic_move_vector}];
        let len = vector::length(&raw_ic);
        let res = vector::empty<Element<G1>>();
        let i = 0;
        while (i < len) {{
            let pt_bytes = vector::borrow(&raw_ic, i);
            let opt = deserialize<G1, FormatG1Compr>(pt_bytes);
            assert!(option::is_some(&opt), E_DESERIALIZATION_FAILED);
            vector::push_back(&mut res, option::extract(&mut opt));
            i = i + 1;
        }};
        res
    }}

    /// Gas-optimized Groth16 verification using precomputed Fq12 pairing and 3-pair multi_pairing
    public fun verify_groth16_prepared(
        proof_a_bytes: &vector<u8>,
        proof_b_bytes: &vector<u8>,
        proof_c_bytes: &vector<u8>,
        public_inputs: &vector<Element<Fr>>
    ): bool {{
        let pvk_alpha_beta = option::extract(&mut deserialize<Fq12, FormatFq12LscLsb>(&PVK_ALPHA_BETA_FQ12));
        let pvk_gamma_neg = option::extract(&mut deserialize<G2, FormatG2Compr>(&PVK_GAMMA_G2_NEG));
        let pvk_delta_neg = option::extract(&mut deserialize<G2, FormatG2Compr>(&PVK_DELTA_G2_NEG));

        let opt_a = deserialize<G1, FormatG1Compr>(proof_a_bytes);
        assert!(option::is_some(&opt_a), E_DESERIALIZATION_FAILED);
        let a = option::extract(&mut opt_a);

        let opt_b = deserialize<G2, FormatG2Compr>(proof_b_bytes);
        assert!(option::is_some(&opt_b), E_DESERIALIZATION_FAILED);
        let b = option::extract(&mut opt_b);

        let opt_c = deserialize<G1, FormatG1Compr>(proof_c_bytes);
        assert!(option::is_some(&opt_c), E_DESERIALIZATION_FAILED);
        let c = option::extract(&mut opt_c);

        let vk_gamma_abc = get_vk_gamma_abc();
        let num_inputs = vector::length(public_inputs);
        assert!(vector::length(&vk_gamma_abc) == num_inputs + 1, E_INPUT_SIZE_MISMATCH);

        // Scalars = [1, pub_0, pub_1, ...]
        let scalars = vector[from_u64<Fr>(1)];
        let j = 0;
        while (j < num_inputs) {{
            vector::push_back(&mut scalars, *vector::borrow(public_inputs, j));
            j = j + 1;
        }};

        let l = multi_scalar_mul(&vk_gamma_abc, &scalars);

        let g1_vec = vector[a, l, c];
        let g2_vec = vector[b, pvk_gamma_neg, pvk_delta_neg];

        let pairing_res = multi_pairing<G1, G2, Gt>(&g1_vec, &g2_vec);
        eq(&pvk_alpha_beta, &upcast(&pairing_res))
    }}

    /// Naive Groth16 verification computing 4 individual pairings without precomputation
    public fun verify_groth16_naive(
        proof_a_bytes: &vector<u8>,
        proof_b_bytes: &vector<u8>,
        proof_c_bytes: &vector<u8>,
        public_inputs: &vector<Element<Fr>>
    ): bool {{
        let vk_alpha = option::extract(&mut deserialize<G1, FormatG1Compr>(&VK_ALPHA_G1));
        let vk_beta = option::extract(&mut deserialize<G2, FormatG2Compr>(&VK_BETA_G2));
        let vk_gamma = option::extract(&mut deserialize<G2, FormatG2Compr>(&VK_GAMMA_G2));
        let vk_delta = option::extract(&mut deserialize<G2, FormatG2Compr>(&VK_DELTA_G2));

        let a = option::extract(&mut deserialize<G1, FormatG1Compr>(proof_a_bytes));
        let b = option::extract(&mut deserialize<G2, FormatG2Compr>(proof_b_bytes));
        let c = option::extract(&mut deserialize<G1, FormatG1Compr>(proof_c_bytes));

        let left = pairing<G1, G2, Gt>(&a, &b);

        let vk_gamma_abc = get_vk_gamma_abc();
        let num_inputs = vector::length(public_inputs);
        let scalars = vector[from_u64<Fr>(1)];
        let j = 0;
        while (j < num_inputs) {{
            vector::push_back(&mut scalars, *vector::borrow(public_inputs, j));
            j = j + 1;
        }};
        let l = multi_scalar_mul(&vk_gamma_abc, &scalars);

        let right = zero<Gt>();
        let right = add(&right, &pairing<G1, G2, Gt>(&vk_alpha, &vk_beta));
        let right = add(&right, &pairing<G1, G2, Gt>(&l, &vk_gamma));
        let right = add(&right, &pairing<G1, G2, Gt>(&c, &vk_delta));

        eq(&left, &right)
    }}

    /// Helper function to verify the embedded proof using prepared Fq12 pairing
    public fun verify_embedded_prepared(): bool {{
        let proof_a = x"{proof_a_hex}";
        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        verify_groth16_prepared(&proof_a, &proof_b, &proof_c, &pub_inputs)
    }}

    /// Helper function to verify the embedded proof using naive 4-pairing verification
    public fun verify_embedded_naive(): bool {{
        let proof_a = x"{proof_a_hex}";
        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        verify_groth16_naive(&proof_a, &proof_b, &proof_c, &pub_inputs)
    }}

    /// Public entry function for transaction execution
    public entry fun verify(
        proof_a_bytes: vector<u8>,
        proof_b_bytes: vector<u8>,
        proof_c_bytes: vector<u8>,
        public_inputs_bytes: vector<vector<u8>>
    ) {{
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&public_inputs_bytes);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&public_inputs_bytes, i);
            let opt = deserialize<Fr, FormatFrLsb>(bytes);
            assert!(option::is_some(&opt), E_DESERIALIZATION_FAILED);
            vector::push_back(&mut pub_inputs, option::extract(&mut opt));
            i = i + 1;
        }};

        let ok = verify_groth16_prepared(
            &proof_a_bytes,
            &proof_b_bytes,
            &proof_c_bytes,
            &pub_inputs
        );
        assert!(ok, E_PROOF_VERIFICATION_FAILED);
    }}

    #[test_only]
    use aptos_std::crypto_algebra::enable_cryptography_algebra_natives;

    #[test(fx = @std)]
    fun test_verify_valid_proof(fx: signer) {{
        enable_cryptography_algebra_natives(&fx);

        let proof_a = x"{proof_a_hex}";
        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        // 1. Prepared verification
        let ok_prepared = verify_groth16_prepared(&proof_a, &proof_b, &proof_c, &pub_inputs);
        assert!(ok_prepared, 101);

        // 2. Naive verification
        let ok_naive = verify_groth16_naive(&proof_a, &proof_b, &proof_c, &pub_inputs);
        assert!(ok_naive, 102);
    }}

    #[test(fx = @std)]
    fun test_negative_tampered_proof(fx: signer) {{
        enable_cryptography_algebra_natives(&fx);

        // Tamper with proof_a by modifying its last byte
        let bad_proof_a = x"{proof_a_hex}";
        let last_idx = vector::length(&bad_proof_a) - 1;
        let b = vector::borrow_mut(&mut bad_proof_a, last_idx);
        *b = *b ^ 0x01; // flip a bit

        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        // Either deserialization fails or verification evaluates to false
        let opt_a = deserialize<G1, FormatG1Compr>(&bad_proof_a);
        if (option::is_some(&opt_a)) {{
            let ok = verify_groth16_prepared(&bad_proof_a, &proof_b, &proof_c, &pub_inputs);
            assert!(!ok, 201);
        }};
    }}

    #[test(fx = @std)]
    fun test_negative_wrong_public_input(fx: signer) {{
        enable_cryptography_algebra_natives(&fx);

        let proof_a = x"{proof_a_hex}";
        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            if (i == 0) {{
                // Tamper with first public input: add 1
                let one = from_u64<Fr>(1);
                fr = add(&fr, &one);
            }};
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        let ok = verify_groth16_prepared(&proof_a, &proof_b, &proof_c, &pub_inputs);
        assert!(!ok, 301);
    }}

    #[test(fx = @std)]
    #[expected_failure(abort_code = E_DESERIALIZATION_FAILED, location = Self)]
    fun test_negative_malformed_point_length(fx: signer) {{
        enable_cryptography_algebra_natives(&fx);

        let malformed_proof_a = x"123456"; // Invalid length (not 32 bytes)
        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        verify_groth16_prepared(&malformed_proof_a, &proof_b, &proof_c, &pub_inputs);
    }}

    #[test(fx = @std)]
    #[expected_failure(abort_code = E_DESERIALIZATION_FAILED, location = Self)]
    fun test_negative_point_not_on_curve(fx: signer) {{
        enable_cryptography_algebra_natives(&fx);

        // Point not on curve: x = 4 (4^3 + 3 = 67 has no square root in Fq)
        let bad_g1 = x"0400000000000000000000000000000000000000000000000000000000000000";
        let proof_b = x"{proof_b_hex}";
        let proof_c = x"{proof_c_hex}";

        let raw_pub: vector<vector<u8>> = vector[{pub_inputs_move_vector}];
        let pub_inputs = vector::empty<Element<Fr>>();
        let len = vector::length(&raw_pub);
        let i = 0;
        while (i < len) {{
            let bytes = vector::borrow(&raw_pub, i);
            let fr = option::extract(&mut deserialize<Fr, FormatFrLsb>(bytes));
            vector::push_back(&mut pub_inputs, fr);
            i = i + 1;
        }};

        verify_groth16_prepared(&bad_g1, &proof_b, &proof_c, &pub_inputs);
    }}
}}
"#
            );

            let move_file_path = out_path.join(format!("{}.move", module_name));
            std::fs::write(&move_file_path, move_code)?;

            println!("Successfully generated deployable Groth16 Move module: {}", move_file_path.display());
            println!("Call arguments written to: {}", call_args_path.display());
            println!("Summary:");
            println!("  Public inputs count: {}", public_json.len());
            println!("  IC elements count: {}", vk_json.ic.len());
            println!("  AlphaBeta Fq12 precomputed bytes: 384 bytes");
            println!("  G2 points (gamma_neg, delta_neg): 64 bytes each");
        }
    }

    Ok(())
}
