use anchor_lang::solana_program::sysvar::instructions::{
    construct_instructions_data, BorrowedInstruction,
};
use anchor_lang::solana_program::{
    account_info::AccountInfo,
    clock::Epoch,
    instruction::{AccountMeta, Instruction},
};
use anchor_lang::{prelude::Pubkey, AnchorSerialize, Space};
use solana_sdk_ids::{ed25519_program, sysvar::instructions::ID as INSTRUCTIONS_SYSVAR_ID};
use solana_sha256_hasher::hash;

use crate::{
    constants::PROVIDER_SIGNER_SEED,
    error::ErrorCode,
    instructions::admin::{validate_initialize_config_authorities, validate_provider_signer_keys},
    state::{ProviderSigner, SubmitReceiptArgs},
    utils::{
        build_compact_receipt_hash, validate_submit_receipt_args,
        verify_preceding_ed25519_instruction, CompactReceiptHashInputs,
    },
};

#[test]
fn submit_receipt_args_serialize_to_deterministic_120_bytes() {
    let args = sample_submit_receipt_args();

    let encoded = args.try_to_vec().unwrap();

    assert_eq!(encoded.len(), 120);
    assert_eq!(encoded, args.try_to_vec().unwrap());
    assert_eq!(encoded, expected_compact_submit_receipt_args_bytes(&args));
}

#[test]
fn compact_receipt_hash_changes_when_fixed_size_input_changes() {
    let mut inputs = sample_compact_hash_inputs();
    let baseline = build_compact_receipt_hash(&inputs);

    inputs.metadata_hash[0] ^= 1;
    let changed = build_compact_receipt_hash(&inputs);

    assert_ne!(baseline, changed);
    assert_eq!(
        baseline,
        build_compact_receipt_hash(&sample_compact_hash_inputs())
    );
}

#[test]
fn submit_receipt_args_validation_rejects_token_sum_overflow() {
    let mut args = sample_submit_receipt_args();
    args.prompt_tokens = u64::MAX;
    args.completion_tokens = 1;

    assert!(validate_submit_receipt_args(&args).is_err());
}

#[test]
fn provider_signer_init_space_includes_signer_pubkey() {
    let legacy_without_signer = 32 + 1 + 1 + 8 + 8;
    assert_eq!(ProviderSigner::INIT_SPACE, legacy_without_signer + 32);
}

#[test]
fn provider_signer_pda_is_wallet_keyed() {
    let provider_wallet = Pubkey::new_from_array([21; 32]);
    let signer = Pubkey::new_from_array([22; 32]);

    let (wallet_keyed_pda, _) = Pubkey::find_program_address(
        &[
            PROVIDER_SIGNER_SEED,
            provider_wallet.as_ref(),
            signer.as_ref(),
        ],
        &crate::ID,
    );

    let provider_code_seed = hash(b"unipass").to_bytes();
    let (legacy_code_keyed_pda, _) = Pubkey::find_program_address(
        &[
            PROVIDER_SIGNER_SEED,
            provider_code_seed.as_ref(),
            signer.as_ref(),
        ],
        &crate::ID,
    );

    assert_ne!(wallet_keyed_pda, legacy_code_keyed_pda);
}

#[test]
fn initialize_config_authority_guards_reject_default_pubkeys() {
    let valid = Pubkey::new_unique();

    let err = validate_initialize_config_authorities(Pubkey::default(), valid, valid).unwrap_err();
    assert_eq!(
        error_code_number(err),
        u32::from(ErrorCode::InvalidAuthority)
    );

    let err = validate_initialize_config_authorities(valid, Pubkey::default(), valid).unwrap_err();
    assert_eq!(
        error_code_number(err),
        u32::from(ErrorCode::InvalidPauseAuthority)
    );

    let err = validate_initialize_config_authorities(valid, valid, Pubkey::default()).unwrap_err();
    assert_eq!(
        error_code_number(err),
        u32::from(ErrorCode::InvalidChallengeResolver)
    );
}

#[test]
fn upsert_provider_signer_key_guards_reject_default_pubkeys() {
    let valid = Pubkey::new_unique();

    let err = validate_provider_signer_keys(Pubkey::default(), valid).unwrap_err();
    assert_eq!(error_code_number(err), u32::from(ErrorCode::InvalidSigner));

    let err = validate_provider_signer_keys(valid, Pubkey::default()).unwrap_err();
    assert_eq!(
        error_code_number(err),
        u32::from(ErrorCode::InvalidProviderWallet)
    );
}

#[test]
fn preceding_ed25519_instruction_must_match_receipt_args() {
    let signer = Pubkey::new_from_array([7; 32]);
    let signature = [9; 64];
    let message = [5; 32];
    let ed25519_ix = Instruction {
        program_id: ed25519_program::id(),
        accounts: vec![],
        data: build_test_ed25519_ix_data(&signer, &signature, &message),
    };
    let submit_ix = Instruction {
        program_id: crate::ID,
        accounts: vec![AccountMeta::new(Pubkey::new_unique(), true)],
        data: vec![0],
    };
    let borrowed = [
        borrow_instruction(&ed25519_ix),
        borrow_instruction(&submit_ix),
    ];
    let mut sysvar_data = construct_instructions_data(&borrowed);
    let len = sysvar_data.len();
    sysvar_data[len - 2..len].copy_from_slice(&1u16.to_le_bytes());

    let key = INSTRUCTIONS_SYSVAR_ID;
    let owner = Pubkey::default();
    let mut lamports = 0;
    let account_info = AccountInfo::new(
        &key,
        false,
        false,
        &mut lamports,
        sysvar_data.as_mut_slice(),
        &owner,
        false,
        Epoch::default(),
    );

    verify_preceding_ed25519_instruction(&account_info, &signer, &message).unwrap();
    assert!(verify_preceding_ed25519_instruction(
        &account_info,
        &Pubkey::new_from_array([8; 32]),
        &message
    )
    .is_err());
}

fn error_code_number(err: anchor_lang::error::Error) -> u32 {
    match err {
        anchor_lang::error::Error::AnchorError(anchor_err) => anchor_err.error_code_number,
        other => panic!("expected AnchorError, got {other:?}"),
    }
}

fn sample_submit_receipt_args() -> SubmitReceiptArgs {
    SubmitReceiptArgs {
        request_nonce_hash: [1; 32],
        metadata_hash: [2; 32],
        prompt_tokens: 123,
        completion_tokens: 456,
        charge_atomic: 1_250_000,
        receipt_hash: [3; 32],
    }
}

fn expected_compact_submit_receipt_args_bytes(args: &SubmitReceiptArgs) -> Vec<u8> {
    let mut expected = Vec::with_capacity(120);
    expected.extend_from_slice(&args.request_nonce_hash);
    expected.extend_from_slice(&args.metadata_hash);
    expected.extend_from_slice(&args.prompt_tokens.to_le_bytes());
    expected.extend_from_slice(&args.completion_tokens.to_le_bytes());
    expected.extend_from_slice(&args.charge_atomic.to_le_bytes());
    expected.extend_from_slice(&args.receipt_hash);
    expected
}

fn sample_compact_hash_inputs() -> CompactReceiptHashInputs {
    CompactReceiptHashInputs {
        request_nonce_hash: [11; 32],
        metadata_hash: [12; 32],
        provider_wallet: Pubkey::new_from_array([13; 32]),
        payer_user: Pubkey::new_from_array([14; 32]),
        usdc_mint: Pubkey::new_from_array([15; 32]),
        prompt_tokens: 512,
        completion_tokens: 1_024,
        charge_atomic: 2_000_000,
    }
}

fn borrow_instruction(ix: &Instruction) -> BorrowedInstruction<'_> {
    BorrowedInstruction {
        program_id: &ix.program_id,
        accounts: ix
            .accounts
            .iter()
            .map(
                |meta| anchor_lang::solana_program::sysvar::instructions::BorrowedAccountMeta {
                    pubkey: &meta.pubkey,
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                },
            )
            .collect(),
        data: ix.data.as_slice(),
    }
}

fn build_test_ed25519_ix_data(
    signer: &Pubkey,
    signature: &[u8; 64],
    message: &[u8; 32],
) -> Vec<u8> {
    let public_key_offset = 16u16;
    let signature_offset = public_key_offset + 32;
    let message_data_offset = signature_offset + 64;

    let mut data = Vec::with_capacity(16 + 32 + 64 + 32);
    data.extend_from_slice(&[1, 0]);
    data.extend_from_slice(&signature_offset.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&public_key_offset.to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(&message_data_offset.to_le_bytes());
    data.extend_from_slice(&(message.len() as u16).to_le_bytes());
    data.extend_from_slice(&u16::MAX.to_le_bytes());
    data.extend_from_slice(signer.as_ref());
    data.extend_from_slice(signature);
    data.extend_from_slice(message);
    data
}
