extern crate alloc;
use alloc::format;
use soroban_sdk::{contracttype, Address, Env, Map, String, Symbol, Vec, panic_with_error};
// use alloc::string::ToString; // Unused import

use crate::config::{ConfigManager, ConfigUtils, ContractConfig, Environment};
use crate::err::Error;
use crate::events::EventEmitter;
use crate::extensions::ExtensionManager;
use crate::fees::{FeeConfig, FeeManager};
use crate::markets::MarketStateManager;
// use crate::resolution::MarketResolutionManager;
use crate::audit_trail::{AuditAction, AuditTrailManager};
use alloc::string::ToString;


/// Admin management system for Predictify Hybrid contract
///
/// This module provides a comprehensive admin system with:
/// - Admin initialization and setup functions
/// - Access control and permission validation
/// - Admin role management and hierarchy
/// - Admin action logging and tracking
/// - Admin helper utilities and testing functions
/// - Admin event emission and monitoring

// ===== ADMIN TYPES =====

/// Admin role enumeration
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum AdminRole {
    /// Super admin with all permissions
    SuperAdmin,
    /// Market admin with market management permissions
    MarketAdmin,
    /// Config admin with configuration permissions
    ConfigAdmin,
    /// Fee admin with fee management permissions
    FeeAdmin,
    /// Read-only admin with view permissions only
    ReadOnlyAdmin,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum AdminError {
    CooldownActive = 101, // Error code for rapid abuse prevention
}

/// Severity level for admin broadcasts
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

// 1. Define the storage key for the last action timestamp
/// Admin permission enumeration
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum AdminPermission {
    /// Initialize contract
    Initialize,
    /// Create markets
    CreateMarket,
    /// Close markets
    CloseMarket,
    /// Finalize markets
    FinalizeMarket,
    /// Extend market duration
    ExtendMarket,
    /// Update fee configuration
    UpdateFees,
    /// Update contract configuration
    UpdateConfig,
    /// Reset configuration
    ResetConfig,
    /// Collect fees
    CollectFees,
    /// Manage disputes
    ManageDispute,
    /// View analytics
    ViewAnalytic,
    /// Emergency actions
    Emergency,
    /// Configuration admin actions (set cooldowns, update oracle admin config)
    ConfigAdmin,
}

/// Admin action record
#[derive(Clone, Debug)]
#[contracttype]
pub struct AdminAction {
    pub admin: Address,
    pub action: String,
    pub target: Option<String>,
    pub parameters: Map<String, String>,
    pub timestamp: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Admin role assignment
#[derive(Clone, Debug)]
#[contracttype]
pub struct AdminRoleAssignment {
    pub admin: Address,
    pub role: AdminRole,
    pub assigned_by: Address,
    pub assigned_at: u64,
    pub permissions: Vec<AdminPermission>,
    pub is_active: bool,
}

/// Admin analytics
#[derive(Clone, Debug)]
#[contracttype]
pub struct AdminAnalytics {
    pub total_admins: u32,
    pub active_admins: u32,
    pub total_actions: u32,
    pub successful_actions: u32,
    pub failed_actions: u32,
    pub action_distribution: Map<String, u32>,
    pub role_distribution: Map<String, u32>,
    pub recent_actions: Vec<AdminAction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum AdminActionType {
    Added,
    Removed,
    RoleUpdated,
    Activated,
    Deactivated,
}

/// Admin analytics result for multi-admin system
#[derive(Clone, Debug)]
#[contracttype]
pub struct AdminAnalyticsResult {
    pub total_admins: u32,
    pub active_admins: u32,
    pub role_distribution: Map<String, u32>,
    pub last_updated: u64,
}

/// Multisig configuration for admin operations
#[derive(Clone, Debug)]
#[contracttype]
pub struct MultisigConfig {
    pub threshold: u32,
    pub total_admins: u32,
    pub enabled: bool,
}

/// Pending admin action requiring multisig approval
#[derive(Clone, Debug)]
#[contracttype]
pub struct PendingAdminAction {
    pub action_id: u64,
    pub action_type: String,
    pub target: Address,
    pub initiator: Address,
    pub approvals: Vec<Address>,
    pub created_at: u64,
    pub expires_at: u64,
    pub executed: bool,
    pub data: Map<String, String>,
}

pub struct AdminFunctions;

pub mod fees;
pub mod governance;
pub mod init;
pub mod pause;
pub mod roles;

pub use fees::*;
pub use governance::*;
pub use init::*;
pub use pause::*;
pub use roles::*;
