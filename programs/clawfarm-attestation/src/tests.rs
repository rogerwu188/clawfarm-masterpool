use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::sysvar::instructions::{
    construct_instructions_data, BorrowedInstruction,
};
use anchor_lang::solana_program::{
    account_info::AccountInfo,
    clock::Epoch,
    instruction::{AccountMeta, Instruction},
};
use solana_sdk_ids::{ed25519_program, sysvar::instructions::ID as INSTRUCTIONS_SYSVAR_ID};

use crate::{
    state::{AttesterType, ProofMode, SubmitReceiptArgs, UsageBasis},
    utils::{build_phase1_canonical_cbor, verify_preceding_ed25519_instruction},
};

#[test]
fn canonical_cbor_omits_non_payload_fields_and_absent_optionals() {
    let args = sample_submit_receipt_args();
    let encoded = build_phase1_canonical_cbor(&args).unwrap();

    assert!(contains_subslice(&encoded, b"proof_mode"));
    assert!(!contains_subslice(&encoded, b"signer"));
    assert!(!contains_subslice(&encoded, b"receipt_hash"));
    assert!(!contains_subslice(&encoded, b"provider_request_id"));
    assert!(!contains_subslice(&encoded, b"http_status"));
}

#[test]
fn canonical_cbor_includes_present_optional_fields() {
    let mut args = sample_submit_receipt_args();
    args.provider_request_id = Some("req_123".to_string());
    args.issued_at = Some(1_711_950_000);
    args.expires_at = Some(1_711_953_600);
    args.http_status = Some(200);
    args.latency_ms = Some(1_840);

    let encoded = build_phase1_canonical_cbor(&args).unwrap();

    assert!(contains_subslice(&encoded, b"provider_request_id"));
    assert!(contains_subslice(&encoded, b"issued_at"));
    assert!(contains_subslice(&encoded, b"expires_at"));
    assert!(contains_subslice(&encoded, b"http_status"));
    assert!(contains_subslice(&encoded, b"latency_ms"));
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

fn sample_submit_receipt_args() -> SubmitReceiptArgs {
    SubmitReceiptArgs {
        version: 1,
        proof_mode: ProofMode::SigLog as u8,
        proof_id: "cap_test_001".to_string(),
        request_nonce: "cfn_test_001".to_string(),
        provider: "unipass".to_string(),
        attester_type: AttesterType::Gateway as u8,
        model: "openai/gpt-4.1".to_string(),
        usage_basis: UsageBasis::ProviderReported as u8,
        prompt_tokens: 123,
        completion_tokens: 456,
        total_tokens: 579,
        charge_atomic: 1_250_000,
        charge_mint: Pubkey::new_unique(),
        provider_request_id: None,
        issued_at: None,
        expires_at: None,
        http_status: None,
        latency_ms: None,
        receipt_hash: [0; 32],
        signer: Pubkey::new_unique(),
    }
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
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
