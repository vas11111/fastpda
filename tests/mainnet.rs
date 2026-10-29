//! Well-known PDAs and associated token accounts of public mainnet protocols:
//! fastpda and solana-pubkey must both derive exactly these addresses and
//! bumps.
//!
//! Each address was derived by reference/pda_reference.py and checked against
//! mainnet on 2026-10-29 with `getAccountInfo`: metadata accounts exist, are
//! owned by the Token Metadata program and name the mint; each program
//! account points at its ProgramData address; token accounts exist, are owned
//! by the Token program and name the mint and owner; the whirlpools are owned
//! by the Whirlpool program. The three authorities hold no data; they are the
//! addresses those protocols publish, and two of them own the token accounts
//! below.

use solana_pubkey::{pubkey, Pubkey};

const TOKEN_PROGRAM: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOCIATED_TOKEN_PROGRAM: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const TOKEN_METADATA_PROGRAM: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const UPGRADEABLE_LOADER: Pubkey = pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");
const JUPITER_V6: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
const ORCA_WHIRLPOOLS: Pubkey = pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
const ORCA_WHIRLPOOLS_CONFIG: Pubkey = pubkey!("2LecshUwdy9xi7meFgHtFJQNSKk4KdTrcpvaB56dP2NQ");
const SPL_STAKE_POOL: Pubkey = pubkey!("SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy");
const JITO_STAKE_POOL: Pubkey = pubkey!("Jito4APyf642JPZPx3hGc6WWJ8zPKtRbRs4P815Awbb");

const USDC: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const USDT: Pubkey = pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
const WRAPPED_SOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

const RAYDIUM_AMM_V4_AUTHORITY: Pubkey = pubkey!("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1");
const JITO_WITHDRAW_AUTHORITY: Pubkey = pubkey!("6iQKfEyhr3bZMotVkW6beNZz5CPAkiwvgV2CTje9pVSS");

/// One expected derivation.
struct Known {
    what: &'static str,
    seeds: Vec<Vec<u8>>,
    program_id: Pubkey,
    address: Pubkey,
    bump: u8,
}

fn known() -> Vec<Known> {
    let key = |k: &Pubkey| k.to_bytes().to_vec();
    let metadata = |what, mint: &Pubkey, address, bump| Known {
        what,
        seeds: vec![
            b"metadata".to_vec(),
            key(&TOKEN_METADATA_PROGRAM),
            key(mint),
        ],
        program_id: TOKEN_METADATA_PROGRAM,
        address,
        bump,
    };
    let program_data = |what, program: &Pubkey, address, bump| Known {
        what,
        seeds: vec![key(program)],
        program_id: UPGRADEABLE_LOADER,
        address,
        bump,
    };
    let whirlpool = |what, tick_spacing: u16, address, bump| Known {
        what,
        seeds: vec![
            b"whirlpool".to_vec(),
            key(&ORCA_WHIRLPOOLS_CONFIG),
            key(&WRAPPED_SOL),
            key(&USDC),
            tick_spacing.to_le_bytes().to_vec(),
        ],
        program_id: ORCA_WHIRLPOOLS,
        address,
        bump,
    };
    let token_account = |what, owner: &Pubkey, mint: &Pubkey, address, bump| Known {
        what,
        seeds: vec![key(owner), key(&TOKEN_PROGRAM), key(mint)],
        program_id: ASSOCIATED_TOKEN_PROGRAM,
        address,
        bump,
    };
    vec![
        // Metaplex Token Metadata accounts: ["metadata", program, mint].
        metadata(
            "USDC metadata",
            &USDC,
            pubkey!("5x38Kp4hvdomTCnCrAny4UtMUt5rQBdB6px2K1Ui45Wq"),
            255,
        ),
        metadata(
            "USDT metadata",
            &USDT,
            pubkey!("8c3zk1t1qt3RU43ckuvPkCS7HLbjJqq3J3Me8ov4aHrp"),
            255,
        ),
        metadata(
            "wrapped SOL metadata",
            &WRAPPED_SOL,
            pubkey!("6dM4TqWyWJsbx7obrdLcviBkTafD5E8av61zfU6jq57X"),
            255,
        ),
        // ProgramData accounts of upgradeable programs: [program id] under the loader.
        program_data(
            "Jupiter v6 ProgramData",
            &JUPITER_V6,
            pubkey!("4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT"),
            254,
        ),
        program_data(
            "Token Metadata ProgramData",
            &TOKEN_METADATA_PROGRAM,
            pubkey!("PwDiXFxQsGra4sFFTT8r1QWRMd4vfumiWC1jfWNfdYT"),
            253,
        ),
        program_data(
            "Raydium AMM v4 ProgramData",
            &RAYDIUM_AMM_V4,
            pubkey!("A7ZG7ByDi8DpzT9Ab7CiXhvgYTJQmaDPJkMDoPitaCQV"),
            255,
        ),
        program_data(
            "Orca Whirlpools ProgramData",
            &ORCA_WHIRLPOOLS,
            pubkey!("CtXfPzz36dH5Ws4UYKZvrQ1Xqzn42ecDW6y8NKuiN8nD"),
            254,
        ),
        // Program authorities.
        Known {
            what: "Raydium AMM v4 authority [\"amm authority\"]",
            seeds: vec![b"amm authority".to_vec()],
            program_id: RAYDIUM_AMM_V4,
            address: RAYDIUM_AMM_V4_AUTHORITY,
            bump: 254,
        },
        Known {
            what: "Jupiter v6 Anchor event authority [\"__event_authority\"]",
            seeds: vec![b"__event_authority".to_vec()],
            program_id: JUPITER_V6,
            address: pubkey!("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf"),
            bump: 255,
        },
        Known {
            what: "Jito stake pool withdraw authority [pool, \"withdraw\"]",
            seeds: vec![key(&JITO_STAKE_POOL), b"withdraw".to_vec()],
            program_id: SPL_STAKE_POOL,
            address: JITO_WITHDRAW_AUTHORITY,
            bump: 253,
        },
        // Orca's SOL/USDC whirlpools: ["whirlpool", config, mint a, mint b, tick spacing as u16 LE].
        whirlpool(
            "Orca SOL/USDC whirlpool, tick spacing 4",
            4,
            pubkey!("Czfq3xZZDmsdGdUyrNLtRhGc47cXcZtLG4crryfu44zE"),
            255,
        ),
        whirlpool(
            "Orca SOL/USDC whirlpool, tick spacing 8",
            8,
            pubkey!("7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm"),
            255,
        ),
        whirlpool(
            "Orca SOL/USDC whirlpool, tick spacing 16",
            16,
            pubkey!("21gTfxAnhUDjJGZJDkTXctGFKT8TeiXx6pN1CEg9K1uW"),
            254,
        ),
        whirlpool(
            "Orca SOL/USDC whirlpool, tick spacing 64",
            64,
            pubkey!("HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ"),
            255,
        ),
        // SPL associated token accounts of program-owned authorities:
        // [owner, token program, mint] under the associated token program.
        token_account(
            "USDC account of the Raydium AMM v4 authority",
            &RAYDIUM_AMM_V4_AUTHORITY,
            &USDC,
            pubkey!("BmeV7UWExZeSboQXYW4biUVEx2SyYDVTdWhHoQEQcUFu"),
            255,
        ),
        token_account(
            "USDT account of the Raydium AMM v4 authority",
            &RAYDIUM_AMM_V4_AUTHORITY,
            &USDT,
            pubkey!("2eE3rzUwZwHdymjQcDhNTHRgWjnFsqA4g3pZvHkcMR2a"),
            254,
        ),
        token_account(
            "wrapped SOL account of the Raydium AMM v4 authority",
            &RAYDIUM_AMM_V4_AUTHORITY,
            &WRAPPED_SOL,
            pubkey!("qx84tiXFKS93Md34ENxNBNu33vPsEJ4BU1o7KYHm6pV"),
            255,
        ),
        token_account(
            "USDC account of the Jito stake pool withdraw authority",
            &JITO_WITHDRAW_AUTHORITY,
            &USDC,
            pubkey!("BZnGteMNjkTP9CeDkJiSe8gCSHxixK7qmpTem9izvg8k"),
            255,
        ),
    ]
}

#[test]
fn well_known_mainnet_addresses_derive_exactly() {
    let known = known();
    assert_eq!(known.len(), 18);
    for k in &known {
        let seeds: Vec<&[u8]> = k.seeds.iter().map(|s| s.as_slice()).collect();
        let expected = Some((k.address, k.bump));
        assert_eq!(
            Pubkey::try_find_program_address(&seeds, &k.program_id),
            expected,
            "solana-pubkey: {}",
            k.what
        );

        let found = fastpda::bytes::try_find_program_address(&seeds, k.program_id.as_array());
        assert_eq!(
            found,
            Some((k.address.to_bytes(), k.bump)),
            "fastpda: {}",
            k.what
        );
        #[cfg(feature = "solana-pubkey")]
        assert_eq!(
            fastpda::try_find_program_address(&seeds, &k.program_id),
            expected,
            "fastpda: {}",
            k.what
        );

        // And create, with the bump.
        let bump = [k.bump];
        let mut with_bump = seeds.clone();
        with_bump.push(&bump);
        assert_eq!(
            fastpda::bytes::create_program_address(&with_bump, k.program_id.as_array()),
            Ok(k.address.to_bytes()),
            "fastpda create: {}",
            k.what,
        );
    }
}
