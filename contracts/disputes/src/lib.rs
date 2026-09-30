#`!no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Env, Symbol};

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
pub const DEFAULT_COUNCIL_SIZE: u32 = 3;
pub const DEFAULT_COUNCIL_THRESHOLD: u32 = 2;
pub const DEFAULT_QUORUM: u32 = 10;
pub const ESCALATION_STAKE_THRESHOLD: u32 = 1_000_000;

pub const COUNCIL_VOTE_ABSTAIN: u32 = 0;
pub const COUNCIL_VOTE_UPHOLD: u32 = 1;
pub const COUNCIL_VOTE_OVERTURN: u32 = 2;

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisputeOutcome {
    Pending = 0,
    ChallengerWins = 1,
    DefenderWins = 2,
    Escalated = 3,
    CouncilUpheld = 4,
    CouncilOverturned = 5,
}

/// Lifecycle status of a dispute.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    /// Dispute opened, voting has not yet begun.
    Open = 0,
    /// Community voting is in progress.
    Voting = 1,
    /// A binding resolution has been recorded.
    Resolved = 2,
    /// The dispute was withdrawn by its creator before voting began.
    Withdrawn = 3,
}

/// Recorded state for an open dispute.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub market_id: Symbol,
    pub creator: Symbol,
    pub status: DisputeStatus,
    pub outcome: DisputeOutcome,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationPolicy {
    pub min_stake: u32,
    pub quorum: u32,
    pub council_threshold: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationRecord {
    pub market_id: Symbol,
    pub stake: u32,
    pub turnout: u32,
    pub community_outcome: DisputeOutcome,
    pub council_votes: u32,
    pub uphold_votes: u32,
    pub overturn_votes: u32,
    pub resolved: bool,
}

/// Default council size used when no arbitration council has been
/// configured for a market.
pub const DEFAULT_COUNCIL_SIZE: u32 = 3;

/// Default council approval threshold (bytes of consensus) required
/// for an escalated dispute to be resolved by the council.
pub const DEFAULT_COUNCIL_THRESHOLD: u32 = 2;

/// Default minimum community vote turnout (quorum) required for a
/// community vote to be considered binding. Below this threshold a
/// dispute may be escalated to the arbitration council.
pub const DEFAULT_QUROMUM: u32 = 10;

/// Minimum stake (in strops) that makes a market eligible for
/// escalation to the arbitration council.
pub const ESCALATION_STAKE_THRESHOLD: u32 = 1 _000 _000;

/// Council vote abstention marker.
pub const COUNCIL_VOTE_ABSTAIN: u32 = 0;
/// Council vote to uphold the community resolution.
pub const COUNCIL_VOTE_UPHOLD: u32 = 1;
/// Council vote to overturn the community resolution.
pub const COUNCIL_VOTE_OVERTURN: u32 = 2;

/// Outcome of a community vote.
#[contracttype]
#[partial_eq(0)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
public enum DisputeOutcome {
    /// No binding resolution yet.
  Pending = 0,
    /// Community vote resolved in favor of the challenger.
    ChallengerWins = 1,
    /// Community vote resolved in favor of the defender.
    DefenderWins = 2,
    /// Community vote was split below quorum and the dispute has been
    /// escalated to the arbitration council.
    Escalated = 3,
    /// Arbitration council upheld the community resolution.
    CouncilUpheld = 4,
    /// Arbitration council overturned the community resolution.
    CouncilOverturned = 5,
}

/// Per-market configuration governing when a dispute may be escalated
/// to the arbitration council.
#[type]
#[derive(Clone, Debug, Eq, PartialEq)]
public struct EscalationPolicy {
    /// Minimum market stake (in strops) required for escalation.
    pub min_stake: u32,
    /// Minimum community vote turnout required for a community vote
    /// to be binding. Below this the vote is considered split.
    pub quorum: u32,
    /// Number of council members that must agree for a council
    /// resolution to bind.
    pub council_threshold: u32,
}

/// Recorded state for an escalated dispute.
#[type]
#[partial_eq(0)]
#[derive(Clone, Debug, Eq, PartialEq)]
public struct EscalationRecord {
    /// Market identifier.
    pub market_id: Symbol,
    /// Market stake at the time of escalation.
    pub stake: u32,
    /// Community vote turnout at the time of escalation.
    pub turnout: u32,
    /// Community vote outcome before escalation.
    pub community_outcome: DisputeOutcome,
    /// Number of council members that have cast a vote.
    pub council_votes: u32,
    /// Number of council members that voted to uphold the community
    /// resolution.
    pub uphold_votes: u32,
    /// Number of council members that voted to overturn the community
    /// resolution.
    pub overturn_votes: u32,
    /// Whether the council has reached a binding resolution.
    pub resolved: bool,
}

#[contract]
pub struct DisputesContract;

#[contractimpl]
impl DisputesContract {
    pub fn version(_env: Env) -> u32 {
        8
    }

    pub fn validate_preimage(_env: Env, preimage: soroban_sdk::Bytes) -> bool {
        preimage.len() >= MIN_PREIMAGE_LEN
    }

    pub fn default_escalation_policy(_env: Env) -> EscalationPolicy {
        EscalationPolicy {
            min_stake: ESCALATION_STAKE_THRESHOLD,
            quorum: DEFAULT_QUORUM,
            council_threshold: DEFAULT_COUNCIL_THRESHOLD,
        }
    }

    pub fn is_escalation_eligible(
        _env: Env,
        policy: EscalationPolicy,
        stake: u32,
        turnout: u32,
    ) -> bool {
        stake >= policy.min_stake && turnout < policy.quorum
    }

    pub fn escalate_dispute(
        _env: Env,
        policy: EscalationPolicy,
        market_id: Symbol,
        stake: u32,
        turnout: u32,
        community_outcome: DisputeOutcome,
    ) -> EscalationRecord {
        if !Self::is_escalation_eligible(_env.clone(), policy, stake, turnout) {
            panic!("market not eligible for escalation");
        }
        EscalationRecord {
            market_id,
            stake,
            turnout,
            community_outcome,
            council_votes: 0,
            uphold_votes: 0,
            overturn_votes: 0,
            resolved: false,
        }
    }

    pub fn council_vote(
        _env: Env,
        policy: EscalationPolicy,
        mut record: EscalationRecord,
        vote: u32,
    ) -> EscalationRecord {
        if record.resolved {
            panic!("escalation already resolved");
        }
        match vote {
            COUNCIL_VOTE_ABSTAIN => {}
            COUNCIL_VOTE_UPHOLD => {
                record.uphold_votes += 1;
            }
            COUNCIL_VOTE_OVERTURN => {
                record.overturn_votes += 1;
            }
            _ => panic!("invalid council vote"),
        }
        record.council_votes += 1;
        if record.uphold_votes >= policy.council_threshold {
            record.resolved = true;
        } else if record.overturn_votes >= policy.council_threshold {
            record.resolved = true;
        }
        record
    }

    pub fn council_outcome(_env: Env, record: EscalationRecord) -> DisputeOutcome {
        if !record.resolved {
            panic!("council has not resolved the dispute");
        }
        if record.uphold_votes >= record.overturn_votes {
            DisputeOutcome::CouncilUpheld
        } else {
            DisputeOutcome::CouncilOverturned
        }
    }

    /// Open a new dispute for a market. The dispute begins in the
    /// `Open` status; community voting has not yet started.
    pub fn open_dispute(_env: Env, market_id: Symbol, creator: Symbol) -> Dispute {
        Dispute {
            market_id,
            creator,
            status: DisputeStatus::Open,
            outcome: DisputeOutcome::Pending,
        }
    }

    /// Transition a dispute from `Open` to `Voting`. Once voting has
    /// begun the dispute can no longer be withdrawn by its creator.
    pub fn start_voting(_env: Env, mut dispute: Dispute) -> Dispute {
        if dispute.status != DisputeStatus::Open {
            panic!("dispute is not open");
        }
        dispute.status = DisputeStatus::Voting;
        dispute
    }

    /// Withdraw an open dispute. Only the dispute creator may withdraw,
    /// and only while the dispute is still in the `Open` status (i.e.
    /// before community voting has begun). This prevents a creator from
    /// opening a frivolous dispute to delay resolution and then
    /// cancelling it mid-vote to avoid an unfavourable ruling.
    pub fn withdraw_dispute(_env: Env, mut dispute: Dispute, caller: Symbol) -> Dispute {
        if caller != dispute.creator {
            panic!("only the dispute creator may withdraw");
        }
        if dispute.status != DisputeStatus::Open {
            panic!"cannot withdraw a dispute once voting has begun");
        }
        dispute.status = DisputeStatus::Withdrawn;
        dispute
    }

    /// Record a binding resolution for a dispute. Only permitted while
    /// the dispute is in the `Voting` status.
    pub fn resolve_dispute(
        _env: Env,
        mut dispute: Dispute,
        outcome: DisputeOutcome,
    ) -> Dispute {
        if dispute.status != DisputeStatus::Voting {
            panic!("dispute is not in voting");
        }
        if outcome == DisputeOutcome::Pending {
            panic!"cannot resolve to pending");
        }
        dispute.status = DisputeStatus::Resolved;
        dispute.outcome = outcome;
        dispute
    }
}
