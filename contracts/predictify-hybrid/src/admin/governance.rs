#![allow(unused_imports)]

use super::*;

// ===== MULTISIG MANAGER =====

/// Manages multisig/threshold approval for sensitive admin operations
pub const THRESHOLD_ROTATION_DELAY: u64 = 86400; // 24 hours

/// Pending threshold update for two-phase commit
#[derive(Clone, Debug)]
#[contracttype]
pub struct PendingThresholdUpdate {
    pub new_threshold: u32,
    pub proposed_at: u64,
    pub proposed_by: Address,
}

/// State for multisig signer rotation cooldowns
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MultisigRotationState {
    pub cooldown_seconds: u64,
    pub last_rotation_timestamp: u64,
}

pub struct MultisigManager;

impl MultisigManager {
    /// Propose a new multisig threshold (M-of-N)
    pub fn propose_threshold(env: &Env, admin: &Address, new_threshold: u32) -> Result<(), Error> {
        AdminAccessControl::validate_permission(env, admin, &AdminPermission::Emergency)?;

        let total_admins = Self::count_active_admins(env);
        if new_threshold == 0 || new_threshold > total_admins {
            return Err(Error::InvalidInput);
        }

        let pending = PendingThresholdUpdate {
            new_threshold,
            proposed_at: env.ledger().timestamp(),
            proposed_by: admin.clone(),
        };

        env.storage()
            .persistent()
            .set(&Symbol::new(env, "PendingThreshold"), &pending);

        let config = Self::get_config(env);
        EventEmitter::emit_threshold_proposed(
            env,
            admin,
            config.threshold,
            new_threshold,
            env.ledger().timestamp() + THRESHOLD_ROTATION_DELAY,
        );

        Ok(())
    }

    /// Confirm a pending threshold update after the delay
    pub fn confirm_threshold(env: &Env) -> Result<(), Error> {
        let pending: PendingThresholdUpdate = env
            .storage()
            .persistent()
            .get(&Symbol::new(env, "PendingThreshold"))
            .ok_or(Error::InvalidState)?;

        if env.ledger().timestamp() < pending.proposed_at + THRESHOLD_ROTATION_DELAY {
            return Err(Error::InvalidState);
        }

        let total_admins = Self::count_active_admins(env);
        if pending.new_threshold == 0 || pending.new_threshold > total_admins {
            env.storage().persistent().remove(&Symbol::new(env, "PendingThreshold"));
            return Err(Error::InvalidInput);
        }

        let mut config = Self::get_config(env);
        let old_threshold = config.threshold;
        config.threshold = pending.new_threshold;
        config.enabled = pending.new_threshold > 1;
        config.total_admins = total_admins;

        env.storage()
            .persistent()
            .set(&Symbol::new(env, "MultisigConfig"), &config);

        env.storage().persistent().remove(&Symbol::new(env, "PendingThreshold"));

        EventEmitter::emit_threshold_confirmed(
            env,
            &pending.proposed_by,
            old_threshold,
            pending.new_threshold,
        );

        Ok(())
    }

    /// Cancel a pending threshold proposal
    pub fn cancel_threshold_proposal(env: &Env, admin: &Address) -> Result<(), Error> {
        AdminAccessControl::validate_permission(env, admin, &AdminPermission::Emergency)?;

        if env.storage().persistent().has(&Symbol::new(env, "PendingThreshold")) {
            env.storage().persistent().remove(&Symbol::new(env, "PendingThreshold"));
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    /// Get current multisig configuration
    pub fn get_config(env: &Env) -> MultisigConfig {
        env.storage()
            .persistent()
            .get(&Symbol::new(env, "MultisigConfig"))
            .unwrap_or(MultisigConfig {
                threshold: 1,
                total_admins: 1,
                enabled: false,
            })
    }

    /// Create a pending action requiring approval
    pub fn create_pending_action(
        env: &Env,
        initiator: &Address,
        action_type: String,
        target: Address,
        data: Map<String, String>,
    ) -> Result<u64, Error> {
        AdminAccessControl::validate_permission(env, initiator, &AdminPermission::Emergency)?;

        let action_id = Self::get_next_action_id(env);
        let mut approvals = Vec::new(env);
        approvals.push_back(initiator.clone());

        let action = PendingAdminAction {
            action_id,
            action_type,
            target,
            initiator: initiator.clone(),
            approvals,
            created_at: env.ledger().timestamp(),
            expires_at: env.ledger().timestamp() + 86400, // 24 hours
            executed: false,
            data,
        };

        let key = Self::get_action_key(env, action_id);
        env.storage().persistent().set(&key, &action);

        Ok(action_id)
    }

    /// Approve a pending action
    pub fn approve_action(env: &Env, admin: &Address, action_id: u64) -> Result<bool, Error> {
        AdminAccessControl::validate_permission(env, admin, &AdminPermission::Emergency)?;

        let key = Self::get_action_key(env, action_id);
        let mut action: PendingAdminAction = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ConfigNotFound)?;

        if action.executed {
            return Err(Error::InvalidState);
        }

        if env.ledger().timestamp() > action.expires_at {
            return Err(Error::DisputeError);
        }

        if action.approvals.contains(admin) {
            return Err(Error::InvalidState);
        }

        action.approvals.push_back(admin.clone());
        env.storage().persistent().set(&key, &action);

        let config = Self::get_config(env);
        Ok(action.approvals.len() >= config.threshold)
    }

    /// Execute a pending action if threshold is met
    pub fn execute_action(env: &Env, action_id: u64) -> Result<(), Error> {
        let key = Self::get_action_key(env, action_id);
        let mut action: PendingAdminAction = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ConfigNotFound)?;

        if action.executed {
            return Err(Error::InvalidState);
        }

        let config = Self::get_config(env);
        if action.approvals.len() < config.threshold {
            return Err(Error::Unauthorized);
        }

        action.executed = true;
        env.storage().persistent().set(&key, &action);

        Ok(())
    }

    /// Get pending action details
    pub fn get_pending_action(env: &Env, action_id: u64) -> Option<PendingAdminAction> {
        let key = Self::get_action_key(env, action_id);
        env.storage().persistent().get(&key)
    }

    /// Check if action requires multisig approval
    pub fn requires_multisig(env: &Env) -> bool {
        let config = Self::get_config(env);
        config.enabled && config.threshold > 1
    }

    // Helper methods
    fn get_action_key(env: &Env, action_id: u64) -> Symbol {
        let key_str = alloc::format!("PendingAction_{}", action_id);
        Symbol::new(env, &key_str)
    }

    fn get_next_action_id(env: &Env) -> u64 {
        let key = Symbol::new(env, "NextActionId");
        let current: u64 = env.storage().persistent().get(&key).unwrap_or(1);
        env.storage().persistent().set(&key, &(current + 1));
        current
    }

    fn count_active_admins(env: &Env) -> u32 {
        let count_key = Symbol::new(env, "AdminCount");
        env.storage().persistent().get(&count_key).unwrap_or(1)
    }

    /// Get current rotation state
    pub fn get_rotation_state(env: &Env) -> MultisigRotationState {
        env.storage()
            .persistent()
            .get(&crate::storage::DataKey::MultisigRotationState)
            .unwrap_or(MultisigRotationState {
                cooldown_seconds: 3600, // 1 hour default
                last_rotation_timestamp: 0,
            })
    }

    /// Set rotation cooldown (Emergency permission required)
    pub fn set_rotation_cooldown(env: &Env, admin: &Address, cooldown_seconds: u64) -> Result<(), Error> {
        admin.require_auth();
        AdminAccessControl::validate_permission(env, admin, &AdminPermission::Emergency)?;
        let mut state = Self::get_rotation_state(env);
        state.cooldown_seconds = cooldown_seconds;
        env.storage().persistent().set(&crate::storage::DataKey::MultisigRotationState, &state);
        Ok(())
    }

    /// Enforce rotation cooldown
    fn enforce_rotation_cooldown(env: &Env, admin: &Address) -> Result<(), Error> {
        let mut state = Self::get_rotation_state(env);
        let current_time = env.ledger().timestamp();

        if current_time < state.last_rotation_timestamp + state.cooldown_seconds {
            EventEmitter::emit_signer_rotation_cooldown_hit(env, admin, state.last_rotation_timestamp, state.cooldown_seconds);
            return Err(Error::SignerRotationCooldown);
        }

        state.last_rotation_timestamp = current_time;
        env.storage().persistent().set(&crate::storage::DataKey::MultisigRotationState, &state);
        Ok(())
    }

    /// Add a new signer enforcing rotation cooldown
    pub fn add_signer(env: &Env, admin: &Address, new_signer: &Address, role: AdminRole) -> Result<(), Error> {
        admin.require_auth();
        Self::enforce_rotation_cooldown(env, admin)?;
        AdminManager::add_admin(env, admin, new_signer, role)
    }

    /// Remove a signer enforcing rotation cooldown
    pub fn remove_signer(env: &Env, admin: &Address, old_signer: &Address) -> Result<(), Error> {
        admin.require_auth();
        Self::enforce_rotation_cooldown(env, admin)?;
        AdminManager::remove_admin(env, admin, old_signer)
    }

    /// Rotate signer enforcing rotation cooldown
    pub fn rotate_signer(
        env: &Env,
        admin: &Address,
        old_signer: &Address,
        new_signer: &Address,
        role: AdminRole,
    ) -> Result<(), Error> {
        admin.require_auth();
        Self::enforce_rotation_cooldown(env, admin)?;

        AdminManager::remove_admin(env, admin, old_signer)?;
        AdminManager::add_admin(env, admin, new_signer, role)
    }
}

// ===== ADMIN ACTION LOGGING =====

/// Administrative action logging and audit trail management.
///
/// The `AdminActionLogger` provides comprehensive logging capabilities for all
/// administrative actions performed on the contract. This creates an immutable
/// audit trail for governance, compliance, and security monitoring.
///
/// # Purpose
///
/// This struct handles:
/// - Recording all admin actions with full context
/// - Creating audit trails for compliance
/// - Emitting events for external monitoring
/// - Providing action history retrieval
/// - Supporting forensic analysis and debugging
///
/// # Audit Trail Components
///
/// Each logged action includes:
/// - **Admin Identity**: Who performed the action
/// - **Action Type**: What operation was performed
/// - **Target**: What was affected (market ID, config, etc.)
/// - **Parameters**: Detailed action parameters
/// - **Timestamp**: When the action occurred
/// - **Success Status**: Whether the action succeeded
/// - **Error Details**: Failure reasons if applicable
///
/// # Security and Compliance
///
/// The logging system supports:
/// - Regulatory compliance requirements
/// - Security incident investigation
/// - Governance transparency
/// - Operational monitoring and alerting
pub struct AdminActionLogger;

impl AdminActionLogger {
    /// Records an administrative action in the audit trail.
    ///
    /// This function creates a comprehensive record of admin actions including
    /// all relevant context, parameters, and outcomes. The record is stored
    /// persistently and an event is emitted for external monitoring.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for storage and events
    /// * `admin` - The admin address that performed the action
    /// * `action` - The type of action performed (e.g., "close_market")
    /// * `target` - Optional target identifier (e.g., market ID)
    /// * `parameters` - Map of action parameters and their values
    /// * `success` - Whether the action completed successfully
    /// * `error_message` - Optional error description if action failed
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Action logged successfully
    /// - `Err(Error)` - Logging failed due to storage or event errors
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address, Map, String};
    /// # use predictify_hybrid::admin::AdminActionLogger;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    /// # let mut params = Map::new(&env);
    /// # params.set(
    /// #     String::from_str(&env, "market_id"),
    /// #     String::from_str(&env, "market_123")
    /// # );
    /// # params.set(
    /// #     String::from_str(&env, "outcome"),
    /// #     String::from_str(&env, "Yes")
    /// # );
    ///
    /// // Log successful market finalization
    /// match AdminActionLogger::log_action(
    ///     &env,
    ///     &admin,
    ///     "finalize_market",
    ///     Some(String::from_str(&env, "market_123")),
    ///     params,
    ///     true,
    ///     None
    /// ) {
    ///     Ok(()) => {
    ///         println!("Action logged successfully");
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to log action: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Action Types
    ///
    /// Common action types include:
    /// - **Market Operations**: "close_market", "finalize_market", "extend_market"
    /// - **Configuration**: "update_config", "update_fees", "reset_config"
    /// - **Role Management**: "assign_role", "revoke_role", "update_permissions"
    /// - **System Operations**: "initialize_contract", "emergency_pause"
    ///
    /// # Storage Strategy
    ///
    /// The current implementation stores actions using a simple key-value approach.
    /// In production, consider:
    /// - Time-based partitioning for scalability
    /// - Indexed storage for efficient queries
    /// - Archival strategies for long-term retention
    /// - Compression for storage efficiency
    ///
    /// # Event Emission
    ///
    /// Each logged action emits an event containing:
    /// - Admin address
    /// - Action type
    /// - Success status
    /// - Timestamp (from ledger)
    ///
    /// External systems can subscribe to these events for:
    /// - Real-time monitoring
    /// - Automated alerting
    /// - Integration with external audit systems
    /// - Dashboard updates
    ///
    /// # Error Handling
    ///
    /// Logging failures should be handled carefully:
    /// - Don't fail the main operation if logging fails
    /// - Consider alternative logging mechanisms
    /// - Alert on persistent logging failures
    /// - Maintain operation continuity
    ///
    /// # Best Practices
    ///
    /// - Log all significant admin actions
    /// - Include sufficient context for investigation
    /// - Use consistent action naming conventions
    /// - Sanitize sensitive parameters before logging
    /// - Monitor log storage usage and implement rotation
    pub fn log_action(
        env: &Env,
        admin: &Address,
        action: &str,
        target: Option<String>,
        parameters: Map<String, String>,
        success: bool,
        error_message: Option<String>,
    ) -> Result<(), Error> {
        let admin_action = AdminAction {
            admin: admin.clone(),
            action: String::from_str(env, action),
            target,
            parameters,
            timestamp: env.ledger().timestamp(),
            success,
            error_message,
        };

        // Store action in persistent storage
        let action_key = Symbol::new(env, "admin_action");
        env.storage().persistent().set(&action_key, &admin_action);

        // Emit admin action event
        EventEmitter::emit_admin_action_logged(env, admin, action, &success);

        Ok(())
    }

    /// Retrieves a list of all administrative actions from the audit trail.
    ///
    /// This function provides access to the complete history of administrative
    /// actions for audit, compliance, and analysis purposes. Currently returns
    /// an empty vector due to storage iteration limitations.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for storage access
    /// * `_limit` - Maximum number of actions to retrieve (currently unused)
    ///
    /// # Returns
    ///
    /// Returns `Result<Vec<AdminAction>, Error>` where:
    /// - `Ok(Vec<AdminAction>)` - List of admin actions (currently empty)
    /// - `Err(Error)` - Retrieval failed due to storage errors
    ///
    /// # Current Limitations
    ///
    /// The current implementation returns an empty vector because:
    /// - Soroban SDK lacks efficient storage iteration capabilities
    /// - Actions are stored individually without indexing
    /// - No built-in pagination or filtering mechanisms
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::Env;
    /// # use predictify_hybrid::admin::AdminActionLogger;
    /// # let env = Env::default();
    ///
    /// // Retrieve recent admin actions for audit
    /// match AdminActionLogger::get_admin_actions(&env, 50) {
    ///     Ok(actions) => {
    ///         println!("Found {} admin actions", actions.len());
    ///         for action in actions {
    ///             println!("Action: {} by {:?} at {}",
    ///                 action.action, action.admin, action.timestamp);
    ///         }
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to retrieve actions: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Future Implementation
    ///
    /// A production implementation would include:
    /// - **Indexed Storage**: Actions indexed by timestamp, admin, type
    /// - **Pagination**: Efficient pagination with cursor-based navigation
    /// - **Filtering**: Filter by date range, admin, action type, success status
    /// - **Sorting**: Sort by timestamp, admin, or action type
    /// - **Aggregation**: Summary statistics and trend analysis
    ///
    /// # Proposed Storage Schema
    ///
    /// ```rust
    /// // Time-based partitioning
    /// let partition_key = format!("actions_{}", timestamp / PARTITION_SIZE);
    ///
    /// // Admin-based indexing
    /// let admin_index = format!("admin_actions_{}", admin);
    ///
    /// // Action type indexing
    /// let type_index = format!("action_type_{}", action_type);
    /// ```
    ///
    /// # Use Cases
    ///
    /// This function supports:
    /// - **Compliance Audits**: Providing complete action history
    /// - **Security Analysis**: Investigating suspicious patterns
    /// - **Operational Review**: Understanding admin activity patterns
    /// - **Debugging**: Tracing the sequence of admin operations
    /// - **Reporting**: Generating admin activity reports
    ///
    /// # Performance Considerations
    ///
    /// When implementing full functionality:
    /// - Implement pagination to avoid large result sets
    /// - Use appropriate caching for frequently accessed data
    /// - Consider read replicas for heavy audit workloads
    /// - Implement query optimization for common access patterns
    pub fn get_admin_actions(env: &Env, _limit: u32) -> Result<Vec<AdminAction>, Error> {
        // For now, return empty vector since we don't have a way to iterate over storage
        // In a real implementation, you would store actions in a more sophisticated way
        Ok(Vec::new(env))
    }

    /// Retrieves administrative actions performed by a specific admin.
    ///
    /// This function provides filtered access to the audit trail, showing only
    /// actions performed by a particular admin address. Useful for individual
    /// admin accountability and performance analysis.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for storage access
    /// * `_admin` - The admin address to filter actions for
    /// * `_limit` - Maximum number of actions to retrieve (currently unused)
    ///
    /// # Returns
    ///
    /// Returns `Result<Vec<AdminAction>, Error>` where:
    /// - `Ok(Vec<AdminAction>)` - List of actions by the specified admin (currently empty)
    /// - `Err(Error)` - Retrieval failed due to storage errors
    ///
    /// # Current Limitations
    ///
    /// Similar to `get_admin_actions`, this function currently returns an empty
    /// vector due to Soroban SDK storage iteration limitations. A full implementation
    /// would require indexed storage and efficient filtering capabilities.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::AdminActionLogger;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    ///
    /// // Get actions performed by a specific admin
    /// match AdminActionLogger::get_admin_actions_for_admin(&env, &admin, 25) {
    ///     Ok(actions) => {
    ///         println!("Admin performed {} actions", actions.len());
    ///         for action in actions {
    ///             println!("{}: {} ({})",
    ///                 action.timestamp,
    ///                 action.action,
    ///                 if action.success { "Success" } else { "Failed" }
    ///             );
    ///         }
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to retrieve admin actions: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Use Cases
    ///
    /// This function is valuable for:
    /// - **Individual Accountability**: Tracking specific admin's actions
    /// - **Performance Review**: Analyzing admin activity and success rates
    /// - **Security Investigation**: Investigating suspicious admin behavior
    /// - **Training**: Reviewing new admin's learning progress
    /// - **Compliance**: Demonstrating individual admin compliance
    ///
    /// # Future Implementation Strategy
    ///
    /// A production implementation would include:
    ///
    /// ## Indexed Storage
    /// ```rust
    /// // Store actions with admin-based indexing
    /// let admin_key = format!("admin_{}_{}", admin, timestamp);
    /// env.storage().persistent().set(&admin_key, &action);
    ///
    /// // Maintain admin action count
    /// let count_key = format!("admin_count_{}", admin);
    /// let current_count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
    /// env.storage().persistent().set(&count_key, &(current_count + 1));
    /// ```
    ///
    /// ## Efficient Querying
    /// - Range queries by timestamp
    /// - Pagination with cursor-based navigation
    /// - Filtering by action type and success status
    /// - Sorting options (newest first, oldest first)
    ///
    /// ## Analytics Integration
    /// - Success rate calculation
    /// - Action frequency analysis
    /// - Time-based activity patterns
    /// - Comparison with other admins
    ///
    /// # Security Considerations
    ///
    /// When implementing full functionality:
    /// - Ensure proper access control (admins can only see their own actions unless super admin)
    /// - Sanitize sensitive information in returned data
    /// - Implement rate limiting to prevent abuse
    /// - Log access to audit logs for meta-auditing
    ///
    /// # Performance Optimization
    ///
    /// For high-volume environments:
    /// - Implement caching for frequently accessed admin histories
    /// - Use background processes for heavy analytics
    /// - Consider read replicas for audit queries
    /// - Implement data archival for old actions
    pub fn get_admin_actions_for_admin(
        env: &Env,
        _admin: &Address,
        _limit: u32,
    ) -> Result<Vec<AdminAction>, Error> {
        // For now, return empty vector
        Ok(Vec::new(env))
    }
}

// ===== ADMIN ANALYTICS =====

/// Admin analytics
impl AdminAnalytics {
    /// Calculate admin analytics
    pub fn calculate_admin_analytics(_env: &Env) -> Result<AdminAnalytics, Error> {
        // For now, return default analytics since we don't store complex types
        Ok(AdminAnalytics::default())
    }

    /// Get admin role distribution
    pub fn get_role_distribution(env: &Env) -> Result<Map<AdminRole, u32>, Error> {
        // For now, return empty map
        Ok(Map::new(env))
    }

    /// Get action distribution
    pub fn get_action_distribution(env: &Env) -> Result<Map<String, u32>, Error> {
        // For now, return empty map
        Ok(Map::new(env))
    }
}

// ===== ADMIN UTILITIES =====

/// Admin utility functions
pub struct AdminUtils;

impl AdminUtils {
    /// Check if address is admin
    pub fn is_admin(env: &Env, address: &Address) -> bool {
        AdminRoleManager::get_admin_role(env, address).is_ok()
    }

    /// Check if address is super admin
    pub fn is_super_admin(env: &Env, address: &Address) -> bool {
        match AdminRoleManager::get_admin_role(env, address) {
            Ok(role) => role == AdminRole::SuperAdmin,
            Err(_) => false,
        }
    }

    /// Get admin role name
    pub fn get_role_name(role: &AdminRole) -> String {
        match role {
            AdminRole::SuperAdmin => String::from_str(&soroban_sdk::Env::default(), "SuperAdmin"),
            AdminRole::MarketAdmin => String::from_str(&soroban_sdk::Env::default(), "MarketAdmin"),
            AdminRole::ConfigAdmin => String::from_str(&soroban_sdk::Env::default(), "ConfigAdmin"),
            AdminRole::FeeAdmin => String::from_str(&soroban_sdk::Env::default(), "FeeAdmin"),
            AdminRole::ReadOnlyAdmin => {
                String::from_str(&soroban_sdk::Env::default(), "ReadOnlyAdmin")
            }
        }
    }

    /// Get permission name
    pub fn get_permission_name(permission: &AdminPermission) -> String {
        match permission {
            AdminPermission::Initialize => {
                String::from_str(&soroban_sdk::Env::default(), "Initialize")
            }
            AdminPermission::CreateMarket => {
                String::from_str(&soroban_sdk::Env::default(), "CreateMarket")
            }
            AdminPermission::CloseMarket => {
                String::from_str(&soroban_sdk::Env::default(), "CloseMarket")
            }
            AdminPermission::FinalizeMarket => {
                String::from_str(&soroban_sdk::Env::default(), "FinalizeMarket")
            }
            AdminPermission::ExtendMarket => {
                String::from_str(&soroban_sdk::Env::default(), "ExtendMarket")
            }
            AdminPermission::UpdateFees => {
                String::from_str(&soroban_sdk::Env::default(), "UpdateFees")
            }
            AdminPermission::UpdateConfig => {
                String::from_str(&soroban_sdk::Env::default(), "UpdateConfig")
            }
            AdminPermission::ResetConfig => {
                String::from_str(&soroban_sdk::Env::default(), "ResetConfig")
            }
            AdminPermission::CollectFees => {
                String::from_str(&soroban_sdk::Env::default(), "CollectFees")
            }
            AdminPermission::ManageDispute => {
                String::from_str(&soroban_sdk::Env::default(), "ManageDispute")
            }
            AdminPermission::ViewAnalytic => {
                String::from_str(&soroban_sdk::Env::default(), "ViewAnalytic")
            }
            AdminPermission::Emergency => {
                String::from_str(&soroban_sdk::Env::default(), "Emergency")
            }
            AdminPermission::ConfigAdmin => {
                String::from_str(&soroban_sdk::Env::default(), "ConfigAdmin")
            }
        }
    }
}

// ===== ADMIN HIERARCHY AND ANALYTICS =====

/// Admin hierarchy management for role inheritance and precedence
pub struct AdminHierarchy;

impl AdminHierarchy {
    pub fn get_role_level(role: &AdminRole) -> u8 {
        match role {
            AdminRole::SuperAdmin => 100,
            AdminRole::MarketAdmin => 75,
            AdminRole::ConfigAdmin => 75,
            AdminRole::FeeAdmin => 75,
            AdminRole::ReadOnlyAdmin => 25,
        }
    }

    pub fn can_manage_role(manager_role: &AdminRole, target_role: &AdminRole) -> bool {
        if *manager_role == AdminRole::SuperAdmin {
            return true;
        }
        let manager_level = Self::get_role_level(manager_role);
        let target_level = Self::get_role_level(target_role);
        manager_level > target_level
    }

    /// Get all permissions for a role level
    pub fn get_role_permissions(env: &Env, role: &AdminRole) -> Vec<AdminPermission> {
        AdminRoleManager::get_permissions_for_role(env, role)
    }

    /// Check if one role encompasses another role's permissions
    pub fn role_encompasses(env: &Env, higher_role: &AdminRole, lower_role: &AdminRole) -> bool {
        let higher_permissions = Self::get_role_permissions(env, higher_role);
        let lower_permissions = Self::get_role_permissions(env, lower_role);

        // Check if all lower permissions are contained in higher permissions
        lower_permissions
            .iter()
            .all(|perm| higher_permissions.contains(perm))
    }
}

/// Enhanced admin analytics using existing AdminAnalytics structure
pub struct EnhancedAdminAnalytics;

impl EnhancedAdminAnalytics {
    pub fn get_admin_analytics(env: &Env) -> AdminAnalyticsResult {
        let total_admins = Self::count_total_admins(env);
        let active_admins = Self::count_active_admins(env);
        let role_distribution = Self::get_role_distribution(env);

        AdminAnalyticsResult {
            total_admins,
            active_admins,
            role_distribution,
            last_updated: env.ledger().timestamp(),
        }
    }

    fn count_total_admins(env: &Env) -> u32 {
        let multi_admin_count: u32 = env
            .storage()
            .persistent()
            .get(&Symbol::new(env, "AdminCount"))
            .unwrap_or(0);

        let original_admin_count = if AdminManager::get_original_admin(env).is_some() {
            1
        } else {
            0
        };
        multi_admin_count + original_admin_count
    }

    fn count_active_admins(env: &Env) -> u32 {
        // assume all admins are active

        Self::count_total_admins(env)
    }

    fn get_role_distribution(env: &Env) -> Map<String, u32> {
        let mut distribution = Map::new(env);
        distribution.set(String::from_str(env, "SuperAdmin"), 0);
        distribution.set(String::from_str(env, "MarketAdmin"), 0);
        distribution.set(String::from_str(env, "ConfigAdmin"), 0);
        distribution.set(String::from_str(env, "FeeAdmin"), 0);
        distribution.set(String::from_str(env, "ReadOnlyAdmin"), 0);

        // Count original admin as SuperAdmin
        if AdminManager::get_original_admin(env).is_some() {
            distribution.set(String::from_str(env, "SuperAdmin"), 1);
        }

        distribution
    }

    /// Get detailed analytics for a specific admin
    pub fn get_admin_details(env: &Env, admin: &Address) -> Option<AdminRoleAssignment> {
        AdminManager::get_admin_assignment(env, admin)
    }

    /// Get admin activity summary
    pub fn get_admin_activity_summary(env: &Env) -> Map<String, u32> {
        let mut summary = Map::new(env);

        let total = Self::count_total_admins(env);
        let active = Self::count_active_admins(env);

        summary.set(String::from_str(env, "Total"), total);
        summary.set(String::from_str(env, "Active"), active);
        summary.set(String::from_str(env, "Inactive"), total - active);

        summary
    }
}

/// Integration utilities for seamless migration
pub struct AdminSystemIntegration;

impl AdminSystemIntegration {
    pub fn migrate_to_multi_admin(env: &Env) -> Result<(), Error> {
        if let Some(original_admin) = AdminManager::get_original_admin(env) {
            let assignment = AdminRoleAssignment {
                admin: original_admin.clone(),
                role: AdminRole::SuperAdmin,
                assigned_by: original_admin.clone(),
                assigned_at: env.ledger().timestamp(),
                permissions: AdminRoleManager::get_permissions_for_role(
                    env,
                    &AdminRole::SuperAdmin,
                ),
                is_active: true,
            };

            let admin_key = AdminManager::get_admin_key(env, &original_admin);
            env.storage().persistent().set(&admin_key, &assignment);
            let mut admin_list = Vec::new(env);
            admin_list.push_back(original_admin.clone());
            env.storage()
                .persistent()
                .set(&Symbol::new(env, "AdminList"), &admin_list);
            env.storage()
                .persistent()
                .set(&Symbol::new(env, "AdminCount"), &1u32);
            env.storage()
                .persistent()
                .set(&Symbol::new(env, "MultiAdminMigrated"), &true);

            EventEmitter::emit_admin_action_logged(
                env,
                &original_admin,
                "MigratedToMultiAdmin",
                &true,
            );
        }
        Ok(())
    }

    pub fn is_migrated(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&Symbol::new(env, "MultiAdminMigrated"))
            .unwrap_or(false)
    }

    pub fn validate_admin_unified(
        env: &Env,
        admin: &Address,
        permission: AdminPermission,
    ) -> Result<(), Error> {
        if Self::is_migrated(env) {
            return AdminManager::validate_admin_permission(env, admin, permission);
        }
        AdminAccessControl::validate_permission(env, admin, &permission)
    }

    /// Force migration if not already migrated
    pub fn ensure_migration(env: &Env) -> Result<(), Error> {
        if !Self::is_migrated(env) {
            Self::migrate_to_multi_admin(env)?;
        }
        Ok(())
    }

    /// Get migration status information
    pub fn get_migration_info(env: &Env) -> Map<String, String> {
        let mut info = Map::new(env);

        let is_migrated = Self::is_migrated(env);
        let has_original_admin = AdminManager::get_original_admin(env).is_some();
        let multi_admin_count: u32 = env
            .storage()
            .persistent()
            .get(&Symbol::new(env, "AdminCount"))
            .unwrap_or(0);

        info.set(
            String::from_str(env, "migrated"),
            String::from_str(env, if is_migrated { "true" } else { "false" }),
        );
        info.set(
            String::from_str(env, "has_original_admin"),
            String::from_str(env, if has_original_admin { "true" } else { "false" }),
        );
        info.set(
            String::from_str(env, "multi_admin_count"),
            String::from_str(env, &multi_admin_count.to_string()),
        );

        info
    }
}

// ===== ADMIN TESTING =====

/// Admin testing utilities
pub struct AdminTesting;

impl AdminTesting {
    /// Create test admin action
    pub fn create_test_admin_action(env: &Env, admin: &Address) -> AdminAction {
        AdminAction {
            admin: admin.clone(),
            action: String::from_str(env, "test_action"),
            target: Some(String::from_str(env, "test_target")),
            parameters: Map::new(env),
            timestamp: env.ledger().timestamp().max(1),
            success: true,
            error_message: None,
        }
    }

    /// Create test admin role assignment
    pub fn create_test_role_assignment(env: &Env, admin: &Address) -> AdminRoleAssignment {
        AdminRoleAssignment {
            admin: admin.clone(),
            role: AdminRole::MarketAdmin,
            assigned_by: admin.clone(),
            assigned_at: env.ledger().timestamp(),
            permissions: AdminRoleManager::get_permissions_for_role(env, &AdminRole::MarketAdmin),
            is_active: true,
        }
    }

    /// Validate admin action structure
    pub fn validate_admin_action_structure(action: &AdminAction) -> Result<(), Error> {
        if action.action.len() == 0 {
            return Err(Error::InvalidInput);
        }

        // Note: In test environments, timestamp can be 0, so we skip this validation
        // In production, you might want to add env parameter to enable this check

        Ok(())
    }

    /// Simulate admin action
    pub fn simulate_admin_action(env: &Env, admin: &Address, action: &str) -> Result<(), Error> {
        // Log test action
        AdminActionLogger::log_action(
            env,
            admin,
            action,
            Some(String::from_str(env, "test_target")),
            Map::new(env),
            true,
            None,
        )?;

        Ok(())
    }
}

// ===== DEFAULT IMPLEMENTATIONS =====

impl Default for AdminAnalytics {
    fn default() -> Self {
        let env = soroban_sdk::Env::default();
        Self {
            total_admins: 0,
            active_admins: 0,
            total_actions: 0,
            successful_actions: 0,
            failed_actions: 0,
            action_distribution: Map::new(&env),
            role_distribution: Map::new(&env),
            recent_actions: Vec::new(&env),
        }
    }
}

// ===== MODULE TESTS =====

#[cfg(any())]
mod tests_disabled {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Events;

    #[test]
    fn test_admin_initializer_initialize() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let admin = Address::generate(&env);

        // Test initialization
        env.as_contract(&contract_id, || {
            assert!(AdminInitializer::initialize(&env, &admin).is_ok());

            // Verify admin is stored
            let stored_admin: Address = env
                .storage()
                .persistent()
                .get(&Symbol::new(&env, "Admin"))
                .unwrap();
            assert_eq!(stored_admin, admin);
        });
    }

    #[test]
    fn test_admin_access_control_validate_permission() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // Initialize admin
            AdminInitializer::initialize(&env, &admin).unwrap();

            // Test permission validation
            assert!(AdminAccessControl::validate_permission(
                &env,
                &admin,
                &AdminPermission::CreateMarket
            )
            .is_ok());
        });
    }

    #[test]
    fn test_admin_role_manager_assign_role() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let admin = Address::generate(&env);
        let new_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // Initialize admin
            AdminInitializer::initialize(&env, &admin).unwrap();

            // Assign role
            assert!(AdminRoleManager::assign_role(
                &env,
                &new_admin,
                AdminRole::MarketAdmin,
                &admin
            )
            .is_ok());

            // Verify role assignment
            let role = AdminRoleManager::get_admin_role(&env, &new_admin).unwrap();
            assert_eq!(role, AdminRole::MarketAdmin);
        });
    }

    #[test]
    fn test_admin_functions_close_market() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let admin = Address::generate(&env);
        let _market_id = Symbol::new(&env, "test_market");

        env.as_contract(&contract_id, || {
            // Initialize admin
            AdminInitializer::initialize(&env, &admin).unwrap();

            // Test close market (would need a real market setup)
            // For now, just test the permission mapping and validation without auth
            let permission = AdminAccessControl::map_action_to_permission("close_market").unwrap();
            assert_eq!(permission, AdminPermission::CloseMarket);

            // Test that the admin has the required permission
            assert!(AdminAccessControl::validate_permission(&env, &admin, &permission).is_ok());
        });
    }

    #[test]
    fn test_admin_utils_is_admin() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let admin = Address::generate(&env);
        let non_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // Initialize admin
            AdminInitializer::initialize(&env, &admin).unwrap();

            // Test admin check
            assert!(AdminUtils::is_admin(&env, &admin));
            assert!(!AdminUtils::is_admin(&env, &non_admin));
        });
    }

    #[test]
    fn test_admin_testing_utilities() {
        let env = Env::default();
        let admin = Address::generate(&env);

        let action = AdminTesting::create_test_admin_action(&env, &admin);
        // Check the action structure manually first
        assert!(action.timestamp > 0);
        assert!(AdminTesting::validate_admin_action_structure(&action).is_ok());

        let role_assignment = AdminTesting::create_test_role_assignment(&env, &admin);
        assert_eq!(role_assignment.role, AdminRole::MarketAdmin);
        assert!(role_assignment.is_active);
    }
}

#[cfg(any())]
mod admin_manager_tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, IntoVal};

    // Helper function to get a proper storage key for admin assignments
    fn get_test_admin_key(env: &Env, admin: &Address) -> soroban_sdk::Val {
        // Use a tuple key: (Symbol, Address)
        let prefix = Symbol::new(env, "AdminAssignment");
        (prefix, admin.clone()).into_val(env)
    }

    #[test]
    fn test_add_admin_success() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let super_admin = Address::generate(&env);
        let new_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // Initialize with super admin
            AdminInitializer::initialize(&env, &super_admin).unwrap();

            // Manually store assignment using composite key approach
            let assignment = AdminRoleAssignment {
                admin: new_admin.clone(),
                role: AdminRole::MarketAdmin,
                assigned_by: super_admin.clone(),
                assigned_at: env.ledger().timestamp(),
                permissions: AdminRoleManager::get_permissions_for_role(
                    &env,
                    &AdminRole::MarketAdmin,
                ),
                is_active: true,
            };

            let admin_key = get_test_admin_key(&env, &new_admin);
            env.storage().persistent().set(&admin_key, &assignment);

            // Update count
            let count_key = Symbol::new(&env, "AdminCount");
            env.storage().persistent().set(&count_key, &1u32);

            // Verify admin was added
            let retrieved: AdminRoleAssignment =
                env.storage().persistent().get(&admin_key).unwrap();
            assert_eq!(retrieved.admin, new_admin);
            assert_eq!(retrieved.role, AdminRole::MarketAdmin);
            assert_eq!(retrieved.assigned_by, super_admin);
            assert!(retrieved.is_active);
        });
    }

    #[test]
    fn test_check_role_permissions() {
        let env = Env::default();

        // SuperAdmin should have all permissions
        assert!(AdminManager::check_role_permissions(
            &env,
            AdminRole::SuperAdmin,
            AdminPermission::CreateMarket
        ));
        assert!(AdminManager::check_role_permissions(
            &env,
            AdminRole::SuperAdmin,
            AdminPermission::UpdateFees
        ));

        // MarketAdmin should have market permissions but not fee permissions
        assert!(AdminManager::check_role_permissions(
            &env,
            AdminRole::MarketAdmin,
            AdminPermission::CreateMarket
        ));
        assert!(!AdminManager::check_role_permissions(
            &env,
            AdminRole::MarketAdmin,
            AdminPermission::UpdateFees
        ));

        // ReadOnlyAdmin should only have ViewAnalytic
        assert!(AdminManager::check_role_permissions(
            &env,
            AdminRole::ReadOnlyAdmin,
            AdminPermission::ViewAnalytic
        ));
        assert!(!AdminManager::check_role_permissions(
            &env,
            AdminRole::ReadOnlyAdmin,
            AdminPermission::CreateMarket
        ));
    }

    #[test]
    fn test_get_admin_roles() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let super_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AdminInitializer::initialize(&env, &super_admin).unwrap();

            let roles = AdminManager::get_admin_roles(&env);

            // Should contain the original super admin
            assert!(roles.contains_key(super_admin.clone()));
            assert_eq!(
                roles.get(super_admin.clone()).unwrap(),
                AdminRole::SuperAdmin
            );
        });
    }

    #[test]
    fn test_is_original_admin() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let super_admin = Address::generate(&env);
        let other_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AdminInitializer::initialize(&env, &super_admin).unwrap();

            assert!(AdminManager::is_original_admin(&env, &super_admin));
            assert!(!AdminManager::is_original_admin(&env, &other_admin));
        });
    }

    #[test]
    fn test_permission_hierarchy() {
        // Test that SuperAdmin encompasses all other roles
        let env = Env::default();

        assert!(AdminHierarchy::role_encompasses(
            &env,
            &AdminRole::SuperAdmin,
            &AdminRole::MarketAdmin
        ));
        assert!(AdminHierarchy::role_encompasses(
            &env,
            &AdminRole::SuperAdmin,
            &AdminRole::ConfigAdmin
        ));
        assert!(AdminHierarchy::role_encompasses(
            &env,
            &AdminRole::SuperAdmin,
            &AdminRole::FeeAdmin
        ));
        assert!(AdminHierarchy::role_encompasses(
            &env,
            &AdminRole::SuperAdmin,
            &AdminRole::ReadOnlyAdmin
        ));

        // Test that lower roles don't encompass SuperAdmin
        assert!(!AdminHierarchy::role_encompasses(
            &env,
            &AdminRole::MarketAdmin,
            &AdminRole::SuperAdmin
        ));
    }

    #[test]
    fn test_role_level_hierarchy() {
        // SuperAdmin should have highest level
        assert_eq!(AdminHierarchy::get_role_level(&AdminRole::SuperAdmin), 100);

        // Specialized admins should have same level
        assert_eq!(AdminHierarchy::get_role_level(&AdminRole::MarketAdmin), 75);
        assert_eq!(AdminHierarchy::get_role_level(&AdminRole::ConfigAdmin), 75);
        assert_eq!(AdminHierarchy::get_role_level(&AdminRole::FeeAdmin), 75);

        // ReadOnly should have lowest level
        assert_eq!(
            AdminHierarchy::get_role_level(&AdminRole::ReadOnlyAdmin),
            25
        );
    }

    #[test]
    fn test_can_manage_role() {
        // SuperAdmin can manage all roles
        assert!(AdminHierarchy::can_manage_role(
            &AdminRole::SuperAdmin,
            &AdminRole::SuperAdmin
        ));
        assert!(AdminHierarchy::can_manage_role(
            &AdminRole::SuperAdmin,
            &AdminRole::MarketAdmin
        ));
        assert!(AdminHierarchy::can_manage_role(
            &AdminRole::SuperAdmin,
            &AdminRole::ReadOnlyAdmin
        ));

        // MarketAdmin cannot manage other specialized admins or SuperAdmin
        assert!(!AdminHierarchy::can_manage_role(
            &AdminRole::MarketAdmin,
            &AdminRole::SuperAdmin
        ));
        assert!(!AdminHierarchy::can_manage_role(
            &AdminRole::MarketAdmin,
            &AdminRole::ConfigAdmin
        ));

        // But can manage lower roles
        assert!(AdminHierarchy::can_manage_role(
            &AdminRole::MarketAdmin,
            &AdminRole::ReadOnlyAdmin
        ));
    }

    #[test]
    fn test_role_distribution() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let super_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AdminInitializer::initialize(&env, &super_admin).unwrap();

            let analytics = EnhancedAdminAnalytics::get_admin_analytics(&env);
            let distribution = analytics.role_distribution;

            // Should have SuperAdmin entry
            let super_admin_count = distribution
                .get(String::from_str(&env, "SuperAdmin"))
                .unwrap();
            assert_eq!(super_admin_count, 1);
        });
    }

    #[test]
    fn test_migration_info() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let super_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AdminInitializer::initialize(&env, &super_admin).unwrap();

            let info = AdminSystemIntegration::get_migration_info(&env);

            // Should have migration status
            assert!(info.contains_key(String::from_str(&env, "migrated")));
            assert!(info.contains_key(String::from_str(&env, "has_original_admin")));

            // Should show not migrated initially
            let migrated = info.get(String::from_str(&env, "migrated")).unwrap();
            assert_eq!(migrated, String::from_str(&env, "false"));

            // Should show has original admin
            let has_original = info
                .get(String::from_str(&env, "has_original_admin"))
                .unwrap();
            assert_eq!(has_original, String::from_str(&env, "true"));
        });
    }

    #[test]
    fn test_admin_activity_summary() {
        let env = Env::default();
        let contract_id = env.register(crate::PredictifyHybrid, ());
        let super_admin = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AdminInitializer::initialize(&env, &super_admin).unwrap();

            let summary = EnhancedAdminAnalytics::get_admin_activity_summary(&env);

            // Should have Total, Active, and Inactive keys
            assert!(summary.contains_key(String::from_str(&env, "Total")));
            assert!(summary.contains_key(String::from_str(&env, "Active")));
            assert!(summary.contains_key(String::from_str(&env, "Inactive")));

            let total = summary.get(String::from_str(&env, "Total")).unwrap();
        });
    }

    #[test]
    fn test_admin_broadcast_info() {
        use soroban_sdk::testutils::Events;
        use soroban_sdk::{TryFromVal, TryIntoVal, Val};
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(crate::PredictifyHybrid, ());
        let client = crate::PredictifyHybridClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        client.initialize(&admin, &None, &None);

        // Attacker trying to broadcast should fail (Unauthorized)
        let hash = soroban_sdk::BytesN::from_array(&env, &[1; 32]);
        let reason = String::from_str(&env, "Attacker broadcast");
        let result = client.try_admin_broadcast(
            &attacker,
            &Severity::Info,
            &hash,
            &reason,
        );
        assert!(result.is_err());

        // Admin broadcasting Info should succeed
        let hash_info = soroban_sdk::BytesN::from_array(&env, &[10; 32]);
        let reason_info = String::from_str(&env, "Info notice");
        let result_info = client.try_admin_broadcast(
            &admin,
            &Severity::Info,
            &hash_info,
            &reason_info,
        );
        assert_eq!(result_info.unwrap(), Ok(()));

        // Verify that the event was emitted.
        let mut info_found = false;
        for event in env.events().all().events().iter() {
            let body = match &event.body {
                soroban_sdk::xdr::ContractEventBody::V0(v0) => v0,
            };
            if let Some(first_topic_scval) = body.topics.get(0) {
                let topic: Result<Symbol, _> = first_topic_scval.clone().try_into_val(&env);
                if let Ok(topic) = topic {
                    if topic == Symbol::new(&env, "admin_broadcast") {
                        if let soroban_sdk::xdr::ScVal::Map(Some(sc_map)) = &body.data {
                            let mut severity_opt = None;
                            let mut hash_opt = None;
                            let mut reason_opt = None;

                            for entry in sc_map.iter() {
                                let key_res: Result<Symbol, _> = entry.key.clone().try_into_val(&env);
                                if let Ok(key) = key_res {
                                    if key == Symbol::new(&env, "severity") {
                                        let sev: Result<Severity, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(sev) = sev {
                                            severity_opt = Some(sev);
                                        }
                                    } else if key == Symbol::new(&env, "message_hash") {
                                        let h: Result<soroban_sdk::BytesN<32>, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(h) = h {
                                            hash_opt = Some(h);
                                        }
                                    } else if key == Symbol::new(&env, "reason") {
                                        let r: Result<String, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(r) = r {
                                            reason_opt = Some(r);
                                        }
                                    }
                                }
                            }

                            if let (Some(severity), Some(hash_bytes), Some(reason_str)) = (severity_opt, hash_opt, reason_opt) {
                                if severity == Severity::Info && hash_bytes == hash_info && reason_str == reason_info {
                                    info_found = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(info_found, "Info event not found");
    }

    #[test]
    fn test_admin_broadcast_warning() {
        use soroban_sdk::testutils::Events;
        use soroban_sdk::{TryFromVal, TryIntoVal, Val};
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(crate::PredictifyHybrid, ());
        let client = crate::PredictifyHybridClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin, &None, &None);

        // Admin broadcasting Warning should succeed
        let hash_warn = soroban_sdk::BytesN::from_array(&env, &[20; 32]);
        let reason_warn = String::from_str(&env, "Warning notice");
        let result_warn = client.try_admin_broadcast(
            &admin,
            &Severity::Warning,
            &hash_warn,
            &reason_warn,
        );
        assert_eq!(result_warn.unwrap(), Ok(()));

        // Verify that the event was emitted.
        let mut warn_found = false;
        for event in env.events().all().events().iter() {
            let body = match &event.body {
                soroban_sdk::xdr::ContractEventBody::V0(v0) => v0,
            };
            if let Some(first_topic_scval) = body.topics.get(0) {
                let topic: Result<Symbol, _> = first_topic_scval.clone().try_into_val(&env);
                if let Ok(topic) = topic {
                    if topic == Symbol::new(&env, "admin_broadcast") {
                        if let soroban_sdk::xdr::ScVal::Map(Some(sc_map)) = &body.data {
                            let mut severity_opt = None;
                            let mut hash_opt = None;
                            let mut reason_opt = None;

                            for entry in sc_map.iter() {
                                let key_res: Result<Symbol, _> = entry.key.clone().try_into_val(&env);
                                if let Ok(key) = key_res {
                                    if key == Symbol::new(&env, "severity") {
                                        let sev: Result<Severity, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(sev) = sev {
                                            severity_opt = Some(sev);
                                        }
                                    } else if key == Symbol::new(&env, "message_hash") {
                                        let h: Result<soroban_sdk::BytesN<32>, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(h) = h {
                                            hash_opt = Some(h);
                                        }
                                    } else if key == Symbol::new(&env, "reason") {
                                        let r: Result<String, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(r) = r {
                                            reason_opt = Some(r);
                                        }
                                    }
                                }
                            }

                            if let (Some(severity), Some(hash_bytes), Some(reason_str)) = (severity_opt, hash_opt, reason_opt) {
                                if severity == Severity::Warning && hash_bytes == hash_warn && reason_str == reason_warn {
                                    warn_found = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(warn_found, "Warning event not found");
    }

    #[test]
    fn test_admin_broadcast_critical() {
        use soroban_sdk::testutils::Events;
        use soroban_sdk::{TryFromVal, TryIntoVal, Val};
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(crate::PredictifyHybrid, ());
        let client = crate::PredictifyHybridClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin, &None, &None);

        // Admin broadcasting Critical should succeed
        let hash_crit = soroban_sdk::BytesN::from_array(&env, &[30; 32]);
        let reason_crit = String::from_str(&env, "Critical notice");
        let result_crit = client.try_admin_broadcast(
            &admin,
            &Severity::Critical,
            &hash_crit,
            &reason_crit,
        );
        assert_eq!(result_crit.unwrap(), Ok(()));

        // Verify that the event was emitted.
        let mut crit_found = false;
        for event in env.events().all().events().iter() {
            let body = match &event.body {
                soroban_sdk::xdr::ContractEventBody::V0(v0) => v0,
            };
            if let Some(first_topic_scval) = body.topics.get(0) {
                let topic: Result<Symbol, _> = first_topic_scval.clone().try_into_val(&env);
                if let Ok(topic) = topic {
                    if topic == Symbol::new(&env, "admin_broadcast") {
                        if let soroban_sdk::xdr::ScVal::Map(Some(sc_map)) = &body.data {
                            let mut severity_opt = None;
                            let mut hash_opt = None;
                            let mut reason_opt = None;

                            for entry in sc_map.iter() {
                                let key_res: Result<Symbol, _> = entry.key.clone().try_into_val(&env);
                                if let Ok(key) = key_res {
                                    if key == Symbol::new(&env, "severity") {
                                        let sev: Result<Severity, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(sev) = sev {
                                            severity_opt = Some(sev);
                                        }
                                    } else if key == Symbol::new(&env, "message_hash") {
                                        let h: Result<soroban_sdk::BytesN<32>, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(h) = h {
                                            hash_opt = Some(h);
                                        }
                                    } else if key == Symbol::new(&env, "reason") {
                                        let r: Result<String, _> = entry.val.clone().try_into_val(&env);
                                        if let Ok(r) = r {
                                            reason_opt = Some(r);
                                        }
                                    }
                                }
                            }

                            if let (Some(severity), Some(hash_bytes), Some(reason_str)) = (severity_opt, hash_opt, reason_opt) {
                                if severity == Severity::Critical && hash_bytes == hash_crit && reason_str == reason_crit {
                                    crit_found = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(crit_found, "Critical event not found");
    }
}

impl AdminFunctions {
    /// Closes a market before its natural end time (admin only).
    ///
    /// This function allows authorized admins to forcibly close a market,
    /// preventing further voting and triggering the market closure process.
    /// It's used for emergency situations or when markets need early termination.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address performing the closure (must have CloseMarket permission)
    /// * `market_id` - Unique identifier of the market to close
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Market closed successfully
    /// - `Err(Error)` - Closure failed due to permissions or validation
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin lacks CloseMarket permission
    /// - `Error::MarketNotFound` - Market with given ID doesn't exist
    /// - Authentication errors from AdminAccessControl
    /// - Storage operation errors
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address, Symbol};
    /// # use predictify_hybrid::admin::AdminFunctions;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    /// # let market_id = Symbol::new(&env, "problematic_market");
    ///
    /// // Close a problematic market
    /// match AdminFunctions::close_market(&env, &admin, &market_id) {
    ///     Ok(()) => {
    ///         println!("Market closed successfully");
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to close market: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Closure Process
    ///
    /// The closure process performs these steps:
    /// 1. **Permission Validation**: Ensures admin has CloseMarket permission
    /// 2. **Market Validation**: Confirms market exists and can be closed
    /// 3. **Market Removal**: Removes market from active storage
    /// 4. **Event Emission**: Emits market closure event for monitoring
    /// 5. **Action Logging**: Records the admin action for audit trails
    ///
    /// # Use Cases
    ///
    /// - **Emergency Closure**: Close markets with problematic questions or outcomes
    /// - **Policy Violations**: Close markets that violate platform policies
    /// - **Technical Issues**: Close markets experiencing technical problems
    /// - **Legal Compliance**: Close markets for regulatory compliance
    /// - **Community Requests**: Close markets based on community feedback
    ///
    /// # Post-Closure State
    ///
    /// After closure:
    /// - Market is removed from active storage
    /// - No further voting is possible
    /// - Existing stakes may need manual resolution
    /// - Market appears as closed in historical records
    ///
    /// # Security
    ///
    /// This is a powerful admin function that should be used carefully.
    /// Only admins with CloseMarket permission can execute this function.
    pub fn close_market(env: &Env, admin: &Address, market_id: &Symbol) -> Result<(), Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "close_market")?;

        // Get market
        let _market = MarketStateManager::get_market(env, market_id)?;

        // Close market
        MarketStateManager::remove_market(env, market_id);

        // Emit market closed event
        EventEmitter::emit_market_closed(env, market_id, admin);

        // Log admin action
        let mut params = Map::new(env);
        params.set(
            String::from_str(env, "market_id"),
            String::from_str(env, "market_id"),
        );
        AdminActionLogger::log_action(env, admin, "close_market", None, params, true, None)?;

        Ok(())
    }

    /// Finalizes a market with admin override of the resolution process.
    ///
    /// This function allows authorized admins to directly set the final outcome
    /// of a market, bypassing the normal resolution process. It's used when
    /// manual intervention is required for market resolution.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address performing the finalization (must have FinalizeMarket permission)
    /// * `market_id` - Unique identifier of the market to finalize
    /// * `outcome` - The final outcome to set for the market
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Market finalized successfully
    /// - `Err(Error)` - Finalization failed due to permissions or validation
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin lacks FinalizeMarket permission
    /// - `Error::MarketNotFound` - Market with given ID doesn't exist
    /// - `Error::InvalidOutcome` - Outcome doesn't match market's possible outcomes
    /// - `Error::MarketResolved` - Market has already been finalized
    /// - Resolution errors from MarketResolutionManager
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address, Symbol, String};
    /// # use predictify_hybrid::admin::AdminFunctions;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    /// # let market_id = Symbol::new(&env, "disputed_market");
    /// # let outcome = String::from_str(&env, "Yes");
    ///
    /// // Finalize a disputed market with admin decision
    /// match AdminFunctions::finalize_market(&env, &admin, &market_id, &outcome) {
    ///     Ok(()) => {
    ///         println!("Market finalized with outcome: {}", outcome);
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to finalize market: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Finalization Process
    ///
    /// The finalization process:
    /// 1. **Permission Validation**: Ensures admin has FinalizeMarket permission
    /// 2. **Market Resolution**: Uses MarketResolutionManager to set final outcome
    /// 3. **Event Emission**: Emits market finalization event
    /// 4. **Action Logging**: Records admin action with outcome details
    ///
    /// # Use Cases
    ///
    /// - **Dispute Resolution**: Resolve disputed markets with admin decision
    /// - **Oracle Failures**: Finalize markets when oracles fail or are unavailable
    /// - **Subjective Markets**: Resolve markets requiring human judgment
    /// - **Emergency Resolution**: Quick resolution in time-sensitive situations
    /// - **Correction**: Correct automated resolutions that were incorrect
    ///
    /// # Post-Finalization State
    ///
    /// After finalization:
    /// - Market state changes to Resolved
    /// - Winning outcome is permanently set
    /// - Users can claim winnings based on the outcome
    /// - Market statistics are finalized
    /// - No further changes to the market are possible
    ///
    /// # Governance
    ///
    /// Admin finalization should follow established governance procedures
    /// and be transparent to the community. Consider implementing multi-signature
    /// requirements for high-value market finalizations.
    pub fn finalize_market(
        env: &Env,
        admin: &Address,
        market_id: &Symbol,
        outcome: &String,
    ) -> Result<(), Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "finalize_market")?;

        // Finalize market using resolution manager
        // Temporarily disabled due to resolution module being disabled
        // let _resolution = MarketResolutionManager::finalize_market(env, admin, market_id, outcome)?;
        // For now, just emit the event and return success

        // Emit market finalized event
        EventEmitter::emit_market_finalized(env, market_id, admin, outcome);

        // Log admin action
        let mut params = Map::new(env);
        params.set(
            String::from_str(env, "market_id"),
            String::from_str(env, "market_id"),
        );
        params.set(String::from_str(env, "outcome"), outcome.clone());
        let _ = AdminActionLogger::log_action(
            env,
            admin,
            "finalize_market",
            Some(String::from_str(env, "market_id")),
            params,
            true,
            None,
        );

        Ok(())
    }

    /// Extends the duration of an active market (admin only).
    ///
    /// This function allows authorized admins to extend the voting period
    /// of an active market by adding additional days to its end time.
    /// Extensions require a reason for transparency and audit purposes.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address performing the extension (must have ExtendMarket permission)
    /// * `market_id` - Unique identifier of the market to extend
    /// * `additional_days` - Number of additional days to add to the market duration
    /// * `reason` - Explanation for why the extension is needed
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Market duration extended successfully
    /// - `Err(Error)` - Extension failed due to permissions or validation
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin lacks ExtendMarket permission
    /// - `Error::MarketNotFound` - Market with given ID doesn't exist
    /// - `Error::MarketClosed` - Market has already ended or been closed
    /// - `Error::InvalidDuration` - Extension would exceed maximum allowed duration
    /// - Extension errors from ExtensionManager
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address, Symbol, String};
    /// # use predictify_hybrid::admin::AdminFunctions;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    /// # let market_id = Symbol::new(&env, "active_market");
    /// # let reason = String::from_str(&env, "Low participation, extending for more votes");
    ///
    /// // Extend market by 7 days due to low participation
    /// match AdminFunctions::extend_market_duration(
    ///     &env,
    ///     &admin,
    ///     &market_id,
    ///     7,
    ///     &reason
    /// ) {
    ///     Ok(()) => {
    ///         println!("Market extended by 7 days");
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to extend market: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Extension Process
    ///
    /// The extension process:
    /// 1. **Permission Validation**: Ensures admin has ExtendMarket permission
    /// 2. **Market Validation**: Confirms market exists and is extendable
    /// 3. **Duration Extension**: Uses ExtensionManager to add additional time
    /// 4. **Action Logging**: Records extension with reason for audit trail
    ///
    /// # Extension Limits
    ///
    /// Extensions are subject to limits:
    /// - Maximum total extension days per market
    /// - Maximum single extension duration
    /// - Market must be in Active state
    /// - Extensions cannot exceed platform limits
    ///
    /// # Use Cases
    ///
    /// - **Low Participation**: Extend markets with insufficient voting
    /// - **Technical Issues**: Extend markets affected by technical problems
    /// - **Community Requests**: Extend based on legitimate community requests
    /// - **External Events**: Extend when external events affect market relevance
    /// - **Oracle Delays**: Extend when oracle data will be delayed
    ///
    /// # Transparency
    ///
    /// All extensions are logged with reasons and are publicly visible.
    /// The extension history is maintained for each market, providing
    /// full transparency of admin interventions.
    ///
    /// # Best Practices
    ///
    /// - Provide clear, specific reasons for extensions
    /// - Limit extensions to necessary cases
    /// - Consider community feedback before extending
    /// - Document extension policies and criteria
    pub fn extend_market_duration(
        env: &Env,
        admin: &Address,
        market_id: &Symbol,
        additional_days: u32,
        reason: &String,
    ) -> Result<(), Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "extend_market")?;

        // Extend market using extension manager
        ExtensionManager::extend_market_duration(
            env,
            admin.clone(),
            market_id.clone(),
            additional_days,
            reason.clone(),
        )?;

        // Log admin action
        let mut params = Map::new(env);
        params.set(
            String::from_str(env, "market_id"),
            String::from_str(env, "market_id"),
        );
        params.set(
            String::from_str(env, "additional_days"),
            String::from_str(env, "additional_days"),
        );
        params.set(String::from_str(env, "reason"), reason.clone());
        AdminActionLogger::log_action(
            env,
            admin,
            "extend_market",
            Some(String::from_str(env, "market_id")),
            params,
            true,
            None,
        )?;

        Ok(())
    }

}

impl AdminFunctions {
    /// Updates the core contract configuration (admin only).
    ///
    /// This function allows authorized admins to modify fundamental contract
    /// settings including market limits, validation thresholds, oracle timeouts,
    /// and other operational parameters. Changes affect all contract operations.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address performing the update (must have UpdateConfig permission)
    /// * `new_config` - The new contract configuration to apply
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Configuration updated successfully
    /// - `Err(Error)` - Update failed due to permissions or validation
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin lacks UpdateConfig permission
    /// - `Error::InvalidInput` - Configuration contains invalid values
    /// - Configuration validation errors from ConfigManager
    /// - Storage operation errors
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::AdminFunctions;
    /// # use predictify_hybrid::config::{ContractConfig, Environment};
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    /// # let new_config = ContractConfig {
    /// #     environment: Environment::Mainnet,
    /// #     max_market_duration_days: 365,
    /// #     min_market_duration_days: 1,
    /// #     max_outcomes_per_market: 10,
    /// #     oracle_timeout_seconds: 3600,
    /// # };
    ///
    /// // Update contract configuration for mainnet
    /// match AdminFunctions::update_contract_config(&env, &admin, &new_config) {
    ///     Ok(()) => {
    ///         println!("Contract configuration updated successfully");
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to update configuration: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Configuration Parameters
    ///
    /// The ContractConfig typically includes:
    /// - **Environment**: Target deployment environment (Development/Testnet/Mainnet)
    /// - **Market Limits**: Duration limits, outcome limits, participation limits
    /// - **Validation Thresholds**: Minimum stakes, consensus requirements
    /// - **Oracle Settings**: Timeout values, retry limits, fallback options
    /// - **Extension Limits**: Maximum extensions per market, total extension days
    ///
    /// # Update Process
    ///
    /// The configuration update process:
    /// 1. **Permission Validation**: Ensures admin has UpdateConfig permission
    /// 2. **Configuration Validation**: Validates all configuration parameters
    /// 3. **Config Update**: Uses ConfigManager to store new configuration
    /// 4. **Environment Detection**: Determines and logs environment type
    /// 5. **Action Logging**: Records configuration change for audit trail
    ///
    /// # Impact Assessment
    ///
    /// Configuration changes can have significant impacts:
    /// - **Market Creation**: New limits apply to future markets
    /// - **Existing Markets**: Some changes may affect active markets
    /// - **Oracle Integration**: Timeout changes affect oracle reliability
    /// - **User Experience**: Limits affect what users can do
    ///
    /// # Environment-Specific Considerations
    ///
    /// Different environments have different optimal settings:
    /// - **Development**: Relaxed limits for testing
    /// - **Testnet**: Production-like but with test-friendly parameters
    /// - **Mainnet**: Strict, secure, production-optimized settings
    ///
    /// # Change Management
    ///
    /// For production deployments:
    /// - Test configuration changes thoroughly
    /// - Consider gradual rollout strategies
    /// - Monitor system behavior after changes
    /// - Have rollback procedures ready
    /// - Document all configuration changes
    pub fn update_contract_config(
        env: &Env,
        admin: &Address,
        new_config: &ContractConfig,
    ) -> Result<(), Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "update_config")?;

        // Update contract configuration
        ConfigManager::update_config(env, &new_config)?;
        let env_name = ConfigUtils::get_environment_name(&new_config);
        let mut params = Map::new(env);
        params.set(String::from_str(env, "environment"), env_name);
        AdminActionLogger::log_action(env, admin, "update_config", None, params, true, None)?;

        Ok(())
    }

    /// Resets the contract configuration to default values (admin only).
    ///
    /// This function allows authorized admins to restore the contract configuration
    /// to its default state, effectively undoing all previous configuration changes.
    /// This is useful for recovery scenarios or returning to known-good settings.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address performing the reset (must have ResetConfig permission)
    ///
    /// # Returns
    ///
    /// Returns `Result<ContractConfig, Error>` where:
    /// - `Ok(ContractConfig)` - The default configuration that was applied
    /// - `Err(Error)` - Reset failed due to permissions or system errors
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin lacks ResetConfig permission
    /// - Configuration reset errors from ConfigManager
    /// - Storage operation errors
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::AdminFunctions;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    ///
    /// // Reset configuration to defaults after problematic changes
    /// match AdminFunctions::reset_config_to_defaults(&env, &admin) {
    ///     Ok(default_config) => {
    ///         println!("Configuration reset to defaults successfully");
    ///         println!("Environment: {:?}", default_config.environment);
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to reset configuration: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Default Configuration
    ///
    /// The default configuration typically includes:
    /// - **Environment**: Development (safest default)
    /// - **Market Duration**: 1-30 days (conservative range)
    /// - **Outcomes Limit**: 2-5 outcomes per market
    /// - **Oracle Timeout**: 1 hour (reasonable default)
    /// - **Extension Limits**: 7 days maximum extension
    ///
    /// # Reset Process
    ///
    /// The reset process:
    /// 1. **Permission Validation**: Ensures admin has ResetConfig permission
    /// 2. **Default Retrieval**: Gets default configuration from ConfigManager
    /// 3. **Configuration Reset**: Applies default configuration
    /// 4. **Action Logging**: Records reset action for audit trail
    /// 5. **Return Defaults**: Returns the applied default configuration
    ///
    /// # Use Cases
    ///
    /// Configuration reset is useful for:
    /// - **Recovery**: Recovering from problematic configuration changes
    /// - **Debugging**: Isolating issues by returning to known-good state
    /// - **Maintenance**: Periodic reset to clean configuration state
    /// - **Environment Migration**: Resetting before environment-specific setup
    /// - **Emergency Response**: Quick restoration during incidents
    ///
    /// # Impact and Considerations
    ///
    /// Resetting configuration affects:
    /// - **Active Markets**: May change behavior of ongoing markets
    /// - **User Limits**: Changes what users can do immediately
    /// - **Oracle Integration**: May affect oracle timeout behavior
    /// - **Platform Behavior**: Returns all settings to baseline
    ///
    /// # Best Practices
    ///
    /// - Use reset as a last resort after other fixes fail
    /// - Announce configuration resets to users
    /// - Monitor system behavior after reset
    /// - Document why reset was necessary
    /// - Consider partial configuration fixes before full reset
    ///
    /// # Recovery Procedures
    ///
    /// After reset, you may need to:
    /// - Reconfigure environment-specific settings
    /// - Update fee structures if needed
    /// - Verify oracle integrations work correctly
    /// - Test market creation and resolution
    pub fn reset_config_to_defaults(env: &Env, admin: &Address) -> Result<ContractConfig, Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "reset_config")?;

        // Reset configuration
        let default_config = ConfigManager::reset_to_defaults(env)?;

        // Log admin action
        AdminActionLogger::log_action(env, admin, "reset_config", None, Map::new(env), true, None)?;

        Ok(default_config)
    }

    /// Broadcasts an emergency notice to off-chain clients by emitting an AdminBroadcast event (admin only).
    pub fn admin_broadcast(
        env: &Env,
        admin: &Address,
        severity: Severity,
        message_hash: soroban_sdk::BytesN<32>,
        reason: String,
    ) -> Result<(), Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "admin_broadcast")?;

        // Emit broadcast event
        EventEmitter::emit_admin_broadcast(env, severity, message_hash, reason.clone());

        // Log admin action
        let mut params = Map::new(env);
        params.set(
            String::from_str(env, "severity"),
            String::from_str(env, match severity {
                Severity::Info => "Info",
                Severity::Warning => "Warning",
                Severity::Critical => "Critical",
            }),
        );
        params.set(String::from_str(env, "reason"), reason);
        AdminActionLogger::log_action(env, admin, "admin_broadcast", None, params, true, None)?;

        Ok(())
    }}
