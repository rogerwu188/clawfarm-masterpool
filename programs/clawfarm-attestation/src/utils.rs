use anchor_lang::{
    prelude::*,
    solana_program::sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked,
    },
};
use solana_sdk_ids::ed25519_program;
use solana_sha256_hasher::hash;

use crate::{
    constants::{
        MAX_KEY_ID_LEN, MAX_MODEL_LEN, MAX_PROOF_ID_LEN, MAX_PROVIDER_LEN,
        MAX_PROVIDER_REQUEST_ID_LEN, MAX_REQUEST_NONCE_LEN,
    },
    error::ErrorCode,
    state::{AttesterType, ProofMode, SubmitReceiptArgs, UsageBasis},
};

pub(crate) fn validate_submit_receipt_args(args: &SubmitReceiptArgs) -> Result<()> {
    require!(args.version == 1, ErrorCode::InvalidVersion);
    require!(
        args.proof_mode == ProofMode::SigLog as u8,
        ErrorCode::InvalidProofMode
    );
    require!(
        args.usage_basis == UsageBasis::ProviderReported as u8,
        ErrorCode::InvalidUsageBasis
    );
    require!(
        args.total_tokens == args.prompt_tokens.saturating_add(args.completion_tokens),
        ErrorCode::InvalidTokenTotals
    );

    validate_proof_id(&args.proof_id)?;
    validate_request_nonce(&args.request_nonce)?;
    validate_provider_code(&args.provider)?;
    validate_model(&args.model)?;
    require!(
        attester_type_label(args.attester_type).is_some(),
        ErrorCode::InvalidAttesterType
    );

    if let Some(provider_request_id) = &args.provider_request_id {
        require!(
            provider_request_id.len() <= MAX_PROVIDER_REQUEST_ID_LEN,
            ErrorCode::StringTooLong
        );
    }
    if let Some(issued_at) = args.issued_at {
        require!(issued_at >= 0, ErrorCode::ReceiptExpired);
    }
    if let Some(expires_at) = args.expires_at {
        require!(expires_at >= 0, ErrorCode::ReceiptExpired);
    }
    if let (Some(issued_at), Some(expires_at)) = (args.issued_at, args.expires_at) {
        require!(expires_at >= issued_at, ErrorCode::ReceiptExpired);
    }
    if let Some(http_status) = args.http_status {
        require!(
            (100..=599).contains(&http_status),
            ErrorCode::InvalidHttpStatus
        );
    }
    Ok(())
}

pub(crate) fn validate_request_nonce(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_REQUEST_NONCE_LEN,
        ErrorCode::InvalidRequestNonce
    );
    require!(
        value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        ErrorCode::InvalidRequestNonce
    );
    Ok(())
}

pub(crate) fn validate_proof_id(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_PROOF_ID_LEN,
        ErrorCode::InvalidProofId
    );
    require!(
        value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-')),
        ErrorCode::InvalidProofId
    );
    Ok(())
}

pub(crate) fn validate_provider_code(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_PROVIDER_LEN,
        ErrorCode::InvalidProvider
    );
    Ok(())
}

pub(crate) fn validate_model(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_MODEL_LEN,
        ErrorCode::InvalidModel
    );
    Ok(())
}

pub(crate) fn validate_key_id(value: &str) -> Result<()> {
    require!(
        !value.is_empty() && value.len() <= MAX_KEY_ID_LEN,
        ErrorCode::StringTooLong
    );
    Ok(())
}

pub(crate) fn request_nonce_seed(request_nonce: &str) -> [u8; 32] {
    hash(request_nonce.as_bytes()).to_bytes()
}

pub(crate) fn provider_signer_seed(provider_code: &str) -> [u8; 32] {
    hash(provider_code.as_bytes()).to_bytes()
}

pub(crate) fn attester_type_mask(attester_type: u8) -> u8 {
    match attester_type {
        0 => 1 << 0,
        1 => 1 << 1,
        2 => 1 << 2,
        _ => 0,
    }
}

fn proof_mode_label(proof_mode: u8) -> Option<&'static str> {
    match proof_mode {
        x if x == ProofMode::SigLog as u8 => Some("sig_log"),
        x if x == ProofMode::SigLogZkReserved as u8 => Some("sig_log_zk"),
        _ => None,
    }
}

fn attester_type_label(attester_type: u8) -> Option<&'static str> {
    match attester_type {
        x if x == AttesterType::Provider as u8 => Some("provider"),
        x if x == AttesterType::Gateway as u8 => Some("gateway"),
        x if x == AttesterType::Hybrid as u8 => Some("hybrid"),
        _ => None,
    }
}

fn usage_basis_label(usage_basis: u8) -> Option<&'static str> {
    match usage_basis {
        x if x == UsageBasis::ProviderReported as u8 => Some("provider_reported"),
        x if x == UsageBasis::ServerEstimatedReserved as u8 => Some("server_estimated"),
        x if x == UsageBasis::HybridReserved as u8 => Some("hybrid"),
        x if x == UsageBasis::TokenizerVerifiedReserved as u8 => Some("tokenizer_verified"),
        _ => None,
    }
}

pub(crate) fn build_phase1_canonical_cbor(args: &SubmitReceiptArgs) -> Result<Vec<u8>> {
    let proof_mode =
        proof_mode_label(args.proof_mode).ok_or_else(|| error!(ErrorCode::InvalidProofMode))?;
    let attester_type = attester_type_label(args.attester_type)
        .ok_or_else(|| error!(ErrorCode::InvalidAttesterType))?;
    let usage_basis =
        usage_basis_label(args.usage_basis).ok_or_else(|| error!(ErrorCode::InvalidUsageBasis))?;

    let mut entries = vec![
        ("version", CanonicalValue::Unsigned(u64::from(args.version))),
        ("proof_mode", CanonicalValue::Text(proof_mode.to_string())),
        ("proof_id", CanonicalValue::Text(args.proof_id.clone())),
        (
            "request_nonce",
            CanonicalValue::Text(args.request_nonce.clone()),
        ),
        ("provider", CanonicalValue::Text(args.provider.clone())),
        (
            "attester_type",
            CanonicalValue::Text(attester_type.to_string()),
        ),
        ("model", CanonicalValue::Text(args.model.clone())),
        ("usage_basis", CanonicalValue::Text(usage_basis.to_string())),
        (
            "prompt_tokens",
            CanonicalValue::Unsigned(args.prompt_tokens),
        ),
        (
            "completion_tokens",
            CanonicalValue::Unsigned(args.completion_tokens),
        ),
        ("total_tokens", CanonicalValue::Unsigned(args.total_tokens)),
        (
            "charge_atomic",
            CanonicalValue::Text(args.charge_atomic.to_string()),
        ),
        (
            "charge_mint",
            CanonicalValue::Text(args.charge_mint.to_string()),
        ),
    ];

    if let Some(provider_request_id) = &args.provider_request_id {
        entries.push((
            "provider_request_id",
            CanonicalValue::Text(provider_request_id.clone()),
        ));
    }
    if let Some(issued_at) = args.issued_at {
        entries.push(("issued_at", CanonicalValue::Signed(issued_at)));
    }
    if let Some(expires_at) = args.expires_at {
        entries.push(("expires_at", CanonicalValue::Signed(expires_at)));
    }
    if let Some(http_status) = args.http_status {
        entries.push((
            "http_status",
            CanonicalValue::Unsigned(u64::from(http_status)),
        ));
    }
    if let Some(latency_ms) = args.latency_ms {
        entries.push(("latency_ms", CanonicalValue::Unsigned(latency_ms)));
    }

    entries.sort_by(|(left_key, _), (right_key, _)| {
        encode_cbor_text(left_key).cmp(&encode_cbor_text(right_key))
    });

    let mut out = Vec::new();
    encode_cbor_major_len(5, entries.len() as u64, &mut out);
    for (key, value) in entries {
        out.extend_from_slice(&encode_cbor_text(key));
        encode_canonical_value(&value, &mut out);
    }
    Ok(out)
}

pub(crate) fn verify_preceding_ed25519_instruction(
    instructions_sysvar: &AccountInfo<'_>,
    signer: &Pubkey,
    message: &[u8; 32],
) -> Result<()> {
    let current_index = load_current_index_checked(instructions_sysvar)
        .map_err(|_| error!(ErrorCode::MissingEd25519Instruction))?;
    require!(current_index > 0, ErrorCode::MissingEd25519Instruction);

    let ix = load_instruction_at_checked(usize::from(current_index - 1), instructions_sysvar)
        .map_err(|_| error!(ErrorCode::MissingEd25519Instruction))?;
    require!(
        ix.program_id == ed25519_program::id(),
        ErrorCode::MissingEd25519Instruction
    );
    require!(
        ix.accounts.is_empty(),
        ErrorCode::Ed25519InstructionMismatch
    );

    let data = ix.data.as_slice();
    require!(data.len() >= 16, ErrorCode::Ed25519InstructionMismatch);
    require!(data[0] == 1, ErrorCode::Ed25519InstructionMismatch);

    let _signature_offset = read_u16_le(data, 2)? as usize;
    let signature_instruction_index = read_u16_le(data, 4)?;
    let public_key_offset = read_u16_le(data, 6)? as usize;
    let public_key_instruction_index = read_u16_le(data, 8)?;
    let message_data_offset = read_u16_le(data, 10)? as usize;
    let message_data_size = read_u16_le(data, 12)? as usize;
    let message_instruction_index = read_u16_le(data, 14)?;

    require!(
        signature_instruction_index == u16::MAX
            && public_key_instruction_index == u16::MAX
            && message_instruction_index == u16::MAX,
        ErrorCode::Ed25519InstructionMismatch
    );
    require!(
        message_data_size == 32,
        ErrorCode::Ed25519InstructionMismatch
    );

    let public_key_bytes = read_slice(data, public_key_offset, 32)?;
    let message_bytes = read_slice(data, message_data_offset, message_data_size)?;

    require!(
        public_key_bytes == signer.as_ref(),
        ErrorCode::Ed25519InstructionMismatch
    );
    require!(
        message_bytes == message.as_slice(),
        ErrorCode::Ed25519InstructionMismatch
    );
    Ok(())
}

fn read_u16_le(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = read_slice(data, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_slice(data: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| error!(ErrorCode::Ed25519InstructionMismatch))?;
    require!(end <= data.len(), ErrorCode::Ed25519InstructionMismatch);
    Ok(&data[offset..end])
}

enum CanonicalValue {
    Unsigned(u64),
    Signed(i64),
    Text(String),
}

fn encode_canonical_value(value: &CanonicalValue, out: &mut Vec<u8>) {
    match value {
        CanonicalValue::Unsigned(value) => encode_cbor_major_len(0, *value, out),
        CanonicalValue::Signed(value) => encode_cbor_signed(*value, out),
        CanonicalValue::Text(value) => out.extend_from_slice(&encode_cbor_text(value)),
    }
}

fn encode_cbor_text(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(9 + bytes.len());
    encode_cbor_major_len(3, bytes.len() as u64, &mut out);
    out.extend_from_slice(bytes);
    out
}

fn encode_cbor_signed(value: i64, out: &mut Vec<u8>) {
    if value >= 0 {
        encode_cbor_major_len(0, value as u64, out);
    } else {
        let encoded = (-1_i128 - i128::from(value)) as u64;
        encode_cbor_major_len(1, encoded, out);
    }
}

fn encode_cbor_major_len(major: u8, len: u64, out: &mut Vec<u8>) {
    match len {
        0..=23 => out.push((major << 5) | (len as u8)),
        24..=0xff => out.extend_from_slice(&[(major << 5) | 24, len as u8]),
        0x100..=0xffff => {
            out.push((major << 5) | 25);
            out.extend_from_slice(&(len as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push((major << 5) | 26);
            out.extend_from_slice(&(len as u32).to_be_bytes());
        }
        _ => {
            out.push((major << 5) | 27);
            out.extend_from_slice(&len.to_be_bytes());
        }
    }
}
