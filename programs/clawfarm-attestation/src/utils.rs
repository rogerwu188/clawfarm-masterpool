use anchor_lang::{
    prelude::*,
    solana_program::sysvar::instructions::{
        load_current_index_checked, load_instruction_at_checked,
    },
};
use solana_sdk_ids::ed25519_program;
use solana_sha256_hasher::hash;

use crate::{
    error::ErrorCode,
    state::{AttesterType, SubmitReceiptArgs},
};

pub(crate) const COMPACT_RECEIPT_DOMAIN_SEPARATOR: &[u8] = b"clawfarm:receipt:v2";

pub(crate) struct CompactReceiptHashInputs {
    pub request_nonce_hash: [u8; 32],
    pub metadata_hash: [u8; 32],
    pub provider_wallet: Pubkey,
    pub payer_user: Pubkey,
    pub usdc_mint: Pubkey,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub charge_atomic: u64,
}

pub(crate) fn validate_submit_receipt_args(args: &SubmitReceiptArgs) -> Result<()> {
    require!(
        args.prompt_tokens
            .checked_add(args.completion_tokens)
            .is_some(),
        ErrorCode::InvalidTokenTotals
    );
    Ok(())
}

pub(crate) fn attester_type_mask(attester_type: AttesterType) -> u8 {
    1u8 << (attester_type as u8)
}

pub(crate) fn build_compact_receipt_hash(inputs: &CompactReceiptHashInputs) -> [u8; 32] {
    let mut preimage =
        Vec::with_capacity(COMPACT_RECEIPT_DOMAIN_SEPARATOR.len() + (32 * 5) + (8 * 3));
    preimage.extend_from_slice(COMPACT_RECEIPT_DOMAIN_SEPARATOR);
    preimage.extend_from_slice(&inputs.request_nonce_hash);
    preimage.extend_from_slice(&inputs.metadata_hash);
    preimage.extend_from_slice(inputs.provider_wallet.as_ref());
    preimage.extend_from_slice(inputs.payer_user.as_ref());
    preimage.extend_from_slice(inputs.usdc_mint.as_ref());
    preimage.extend_from_slice(&inputs.prompt_tokens.to_le_bytes());
    preimage.extend_from_slice(&inputs.completion_tokens.to_le_bytes());
    preimage.extend_from_slice(&inputs.charge_atomic.to_le_bytes());

    hash(preimage.as_slice()).to_bytes()
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
