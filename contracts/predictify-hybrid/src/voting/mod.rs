//! Voting domain API.
//!
//! Public types and managers remain available here for backwards
//! compatibility. Internally, the implementation is grouped into focused
//! submodule facades for recording, tallying, quorum, and delegation.

mod legacy;

pub mod record {
    pub use super::legacy::{Vote, VotingManager, VotingValidator};
}

pub mod tally {
    pub use super::legacy::{PayoutData, ThresholdUtils, VotingAnalytics, VotingStats};
}

pub mod quorum {
    pub use super::legacy::{
        DisputeThreshold, ThresholdAdjustmentFactors, ThresholdHistoryEntry,
        ThresholdValidator,
    };
}

pub mod delegation {
    pub use super::legacy::{VotingManager, VotingValidator};
}

pub use legacy::*;
