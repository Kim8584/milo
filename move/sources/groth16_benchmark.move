#[test_only]
module deployer::groth16_benchmark {
    use aptos_std::crypto_algebra::enable_cryptography_algebra_natives;
    use deployer::multiplier2_verifier;
    use deployer::poly4_verifier;

    #[test(fx = @std)]
    fun bench_multiplier2_prepared_20x(fx: signer) {
        enable_cryptography_algebra_natives(&fx);
        let i = 0;
        while (i < 20) {
            let ok = multiplier2_verifier::verify_embedded_prepared();
            assert!(ok, 1);
            i = i + 1;
        };
    }

    #[test(fx = @std)]
    fun bench_multiplier2_naive_20x(fx: signer) {
        enable_cryptography_algebra_natives(&fx);
        let i = 0;
        while (i < 20) {
            let ok = multiplier2_verifier::verify_embedded_naive();
            assert!(ok, 1);
            i = i + 1;
        };
    }

    #[test(fx = @std)]
    fun bench_poly4_prepared_20x(fx: signer) {
        enable_cryptography_algebra_natives(&fx);
        let i = 0;
        while (i < 20) {
            let ok = poly4_verifier::verify_embedded_prepared();
            assert!(ok, 1);
            i = i + 1;
        };
    }

    #[test(fx = @std)]
    fun bench_poly4_naive_20x(fx: signer) {
        enable_cryptography_algebra_natives(&fx);
        let i = 0;
        while (i < 20) {
            let ok = poly4_verifier::verify_embedded_naive();
            assert!(ok, 1);
            i = i + 1;
        };
    }
}
