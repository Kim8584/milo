# Milo

**Milo** is a Proof-to-Move compiler that turns Circom / snarkjs (Groth16 on BN254) verification keys, proofs, and public inputs into **ready-to-deploy, gas-optimized Move verifiers on Aptos**.

*(Noir support is on the roadmap — currently Groth16 / Circom is implemented and tested.)*

---

## Why Milo?

Writing zero-knowledge proof verifiers in Move by hand is tedious, error-prone, and easy to misconfigure (endianness, compressed point formats, subgroup checks). Calling Aptos pairing primitives naively also wastes more than double the gas.

Milo automates the entire workflow in one CLI command:
1. **Validates off-chain first** using Arkworks (`ark-bn254`) before emitting any Move code.
2. **Precomputes fixed key components** off-chain ($e(\alpha, \beta) \in F_{q^{12}}$, $-\gamma \in G_2$, $-\delta \in G_2$).
3. **Emits a standalone Move module** and serialized `call_args.json` ready for Aptos deployment.

---

## Why It's >50% Cheaper (Precomputation + `multi_pairing`)

Groth16 verification checks:

$$e(A, B) = e(\alpha, \beta) \cdot e(L, \gamma) \cdot e(C, \delta)$$

A naive verifier calls `pairing()` four separate times, paying for **4 Miller loops and 4 Final Exponentiations** (`4 × 385.44 = 1,541.74` gas units in pairings alone).

Milo reduces this by **~58.6%** using two optimizations together:
1. **Off-chain precomputation (4 pairs $\to$ 3 pairs):** $e(\alpha, \beta)$ is constant for a circuit, so Milo computes it once off-chain and embeds it in the Move module.
2. **Batched `multi_pairing` (4 Final Exponentiations $\to$ 1):** Milo combines the remaining 3 pairs into a single `crypto_algebra::multi_pairing` call—sharing one Miller loop and running **only one Final Exponentiation** (`234.89 + 3 × 124.29 = 607.77` gas units).

---

## Measured Benchmarks (Aptos Move VM, BN254)

| Circuit | Public Inputs | Mode | Pairing Gas | Total Gas Units | Bytecode Size |
|---|:---:|---|:---:|:---:|:---:|
| `multiplier2` | 1 | **Prepared (Milo)** | **607.77** | **~661** | 4,579 B (4.58 KB) |
| `multiplier2` | 1 | Naive | 1,541.74 | ~1,595 | 4,579 B (4.58 KB) |
| `poly4` | 4 | **Prepared (Milo)** | **607.77** | **~709** | 4,773 B (4.77 KB) |
| `poly4` | 4 | Naive | 1,541.74 | ~1,643 | 4,773 B (4.77 KB) |

- **Module size:** ~4.6 KB (~7% of Aptos's 64 KB module limit).
- **Security & negative tests:** 14/14 passing (`aptos move test`), covering valid proofs, tampered proofs, wrong public inputs, malformed point lengths, and off-curve point rejections.

---

## Roadmap

- [x] **Circom / snarkjs Groth16 (BN254)** — CLI generator (`verify` & `generate`), precomputed prepared verification, negative test suite.
- [ ] **TypeScript / Aptos SDK integration** — One-command `circom -> proof -> aptos deploy`.
- [ ] **Noir (UltraHonk)** — Planned automated verifier generator for Aptos Move.
