#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

/// Minimum byte length required for a commit-reveal preimage.
///
/// The commit-reveal scheme is only binding if the preimage cannot be
/// recovered by an offline brute-force search before the apply window
/// opens. A short preimage (e.g. the 11-byte corpus prefix previously
/// used by the fuzz harness) is trivially enumerable, letting an
/// observer derive the preimage from the commitment and front-run the
/// reveal. Requiring at least 32 bytes of entropy makes such an offline
/// search infeasible.
pub const MIN_PREIMAGE_LEN: u32 = 32;

#[contract]
pub struct DisputesContract;

#[contractimpl]
impl DisputesContract {
    pub fn version(_env: Env) -> u32 {
        7
    }

    /// Validate a commit-reveal preimage before it is committed or
    /// revealed. Rejects preimages shorter than `MIN_PREIMAGE_LEN` so
    /// that offline preimage search before the apply window is
    /// infeasible.
    pub fn validate_preimage(_env: Env, preimage: soroban_sdk::Bytes) -> bool {
        preimage.len() >= MIN_PREIMAGE_LEN
    }
}
