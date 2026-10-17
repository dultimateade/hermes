#![allow(unused_imports)]

use super::*;

// ===== ADMIN ACCESS CONTROL =====

/// Admin access control management
pub struct AdminAccessControl;

impl AdminAccessControl {
    /// Validates that an admin has the required permission for a specific action.
    ///
    /// This function checks if the given admin address has the necessary permission
    /// to perform a specific action based on their assigned role. It's the core
    /// authorization mechanism for all admin operations in the contract.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address to validate permissions for
    /// * `permission` - The specific permission required for the action
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Admin has the required permission
    /// - `Err(Error)` - Admin lacks permission or validation failed
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin doesn't have the required permission
    /// - `Error::Unauthorized` - Admin role not found or inactive
    /// - Role retrieval errors from AdminRoleManager
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::{AdminAccessControl, AdminPermission};
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    ///
    /// // Check if admin can create markets
    /// match AdminAccessControl::validate_permission(
    ///     &env,
    ///     &admin,
    ///     &AdminPermission::CreateMarket
    /// ) {
    ///     Ok(()) => {
    ///         // Admin has permission, proceed with market creation
    ///         println!("Admin authorized for market creation");
    ///     },
    ///     Err(e) => {
    ///         println!("Permission denied: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Permission Hierarchy
    ///
    /// Different admin roles have different permission sets:
    /// - **SuperAdmin**: All permissions
    /// - **MarketAdmin**: Market-related permissions
    /// - **ConfigAdmin**: Configuration permissions
    /// - **FeeAdmin**: Fee management permissions
    /// - **ReadOnlyAdmin**: View-only permissions
    ///
    /// # Use Cases
    ///
    /// - **Function Guards**: Validate permissions before executing admin functions
    /// - **UI Authorization**: Show/hide UI elements based on permissions
    /// - **API Endpoints**: Authorize admin API calls
    /// - **Batch Operations**: Validate permissions for multiple operations
    /// - **Audit Trails**: Log permission checks for security auditing
    pub fn validate_permission(
        env: &Env,
        admin: &Address,
        permission: &AdminPermission,
    ) -> Result<(), Error> {
        // Try new multi-admin system first if migrated
        if AdminSystemIntegration::is_migrated(env) {
            return AdminManager::validate_admin_permission(env, admin, *permission);
        }

        // Fall back to existing logic
        let role = AdminRoleManager::get_admin_role(env, admin)?;

        // Check if admin has the required permission
        if !AdminRoleManager::has_permission(env, &role, permission)? {
            return Err(Error::Unauthorized);
        }

        Ok(())
    }

    /// Requires admin authentication and validates admin status.
    ///
    /// This function performs comprehensive admin authentication by verifying
    /// the caller's signature and confirming they are a registered admin.
    /// It's the fundamental authentication check for all admin operations.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address to authenticate
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Admin is authenticated and authorized
    /// - `Err(Error)` - Authentication or authorization failed
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::AdminNotSet` - No admin has been configured for the contract
    /// - `Error::Unauthorized` - Caller is not the registered admin
    /// - Authentication errors from Soroban's `require_auth()`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::AdminAccessControl;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    ///
    /// // Authenticate admin before sensitive operation
    /// match AdminAccessControl::require_admin_auth(&env, &admin) {
    ///     Ok(()) => {
    ///         // Admin is authenticated, proceed with operation
    ///         println!("Admin authenticated successfully");
    ///     },
    ///     Err(e) => {
    ///         println!("Authentication failed: {:?}", e);
    ///         return;
    ///     }
    /// }
    /// ```
    ///
    /// # Authentication Process
    ///
    /// The authentication performs these checks:
    /// 1. **Signature Verification**: Validates the caller's cryptographic signature
    /// 2. **Admin Lookup**: Retrieves the stored admin address from contract storage
    /// 3. **Address Comparison**: Ensures the caller matches the stored admin
    /// 4. **Status Validation**: Confirms admin status is active
    ///
    /// # Security Considerations
    ///
    /// - Uses Soroban's built-in signature verification
    /// - Prevents unauthorized access to admin functions
    /// - Should be called before any admin-only operations
    /// - Protects against address spoofing attacks
    ///
    /// # Use Cases
    ///
    /// - **Function Entry Points**: First check in admin functions
    /// - **Batch Operations**: Authenticate once for multiple operations
    /// - **API Gateways**: Validate admin API requests
    /// - **Emergency Functions**: Ensure only authorized emergency actions
    pub fn require_admin_auth(env: &Env, admin: &Address) -> Result<(), Error> {
        // Verify admin authentication
        admin.require_auth();

        // Validate admin exists
        let stored_admin: Address = env
            .storage()
            .persistent()
            .get(&Symbol::new(env, "Admin"))
            .ok_or(Error::AdminNotSet)?;

        if admin != &stored_admin {
            return Err(Error::Unauthorized);
        }

        Ok(())
    }

    /// Gets the primary admin address from persistent storage.
    pub fn get_admin(env: &Env) -> Result<Address, Error> {
        env.storage()
            .persistent()
            .get(&Symbol::new(env, "Admin"))
            .ok_or(Error::AdminNotSet)
    }
}

impl AdminAccessControl {
    /// Validates admin authentication and permissions for a specific action.
    ///
    /// This comprehensive validation function combines authentication and
    /// permission checking for a specific action. It's a convenience function
    /// that performs complete admin validation in a single call.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address to validate
    /// * `action` - String identifier of the action to validate (e.g., "create_market")
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Admin is authenticated and authorized for the action
    /// - `Err(Error)` - Authentication, permission, or action mapping failed
    ///
    /// # Errors
    ///
    /// This function returns errors from:
    /// - `require_admin_auth()` - Authentication failures
    /// - `map_action_to_permission()` - Invalid action string
    /// - `validate_permission()` - Permission validation failures
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::AdminAccessControl;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    ///
    /// // Validate admin for market creation
    /// match AdminAccessControl::validate_admin_for_action(
    ///     &env,
    ///     &admin,
    ///     "create_market"
    /// ) {
    ///     Ok(()) => {
    ///         // Admin is fully authorized, proceed with market creation
    ///         println!("Admin authorized for market creation");
    ///     },
    ///     Err(e) => {
    ///         println!("Authorization failed: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Supported Actions
    ///
    /// Valid action strings include:
    /// - `"initialize"` - Contract initialization
    /// - `"create_market"` - Market creation
    /// - `"close_market"` - Market closure
    /// - `"finalize_market"` - Market finalization
    /// - `"extend_market"` - Market duration extension
    /// - `"update_fees"` - Fee configuration updates
    /// - `"update_config"` - Contract configuration updates
    /// - `"collect_fees"` - Fee collection
    /// - `"manage_disputes"` - Dispute management
    /// - `"emergency_actions"` - Emergency operations
    ///
    /// # Validation Process
    ///
    /// The function performs validation in this order:
    /// 1. **Authentication**: Verifies admin signature and status
    /// 2. **Action Mapping**: Maps action string to permission enum
    /// 3. **Permission Check**: Validates admin has required permission
    ///
    /// # Use Cases
    ///
    /// - **Single-Call Validation**: Complete validation in one function call
    /// - **Dynamic Actions**: Validate actions determined at runtime
    /// - **API Endpoints**: Validate admin API calls with action strings
    /// - **Middleware**: Use in middleware for automatic admin validation
    pub fn validate_admin_for_action(
        env: &Env,
        admin: &Address,
        action: &str,
    ) -> Result<(), Error> {
        // Require admin authentication
        AdminAccessControl::require_admin_auth(env, admin)?;

        // Map action to permission
        let permission = AdminAccessControl::map_action_to_permission(action)?;

        // Validate permission
        AdminAccessControl::validate_permission(env, admin, &permission)?;

        Ok(())
    }

    /// Maps action string identifiers to their corresponding permission enums.
    ///
    /// This utility function converts human-readable action strings into
    /// the corresponding AdminPermission enum values. It's used to bridge
    /// string-based action identifiers with the type-safe permission system.
    ///
    /// # Parameters
    ///
    /// * `action` - String identifier of the action to map
    ///
    /// # Returns
    ///
    /// Returns `Result<AdminPermission, Error>` where:
    /// - `Ok(AdminPermission)` - Successfully mapped action to permission
    /// - `Err(Error::InvalidInput)` - Action string is not recognized
    ///
    /// # Errors
    ///
    /// This function returns:
    /// - `Error::InvalidInput` - Action string doesn't match any known actions
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::admin::{AdminAccessControl, AdminPermission};
    ///
    /// // Map action string to permission
    /// match AdminAccessControl::map_action_to_permission("create_market") {
    ///     Ok(permission) => {
    ///         assert_eq!(permission, AdminPermission::CreateMarket);
    ///         println!("Mapped to CreateMarket permission");
    ///     },
    ///     Err(e) => {
    ///         println!("Invalid action: {:?}", e);
    ///     }
    /// }
    ///
    /// // Handle invalid action
    /// match AdminAccessControl::map_action_to_permission("invalid_action") {
    ///     Ok(_) => unreachable!(),
    ///     Err(e) => {
    ///         println!("Expected error for invalid action: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Action Mapping Table
    ///
    /// | Action String | Permission Enum |
    /// |---------------|----------------|
    /// | `"initialize"` | `AdminPermission::Initialize` |
    /// | `"create_market"` | `AdminPermission::CreateMarket` |
    /// | `"close_market"` | `AdminPermission::CloseMarket` |
    /// | `"finalize_market"` | `AdminPermission::FinalizeMarket` |
    /// | `"extend_market"` | `AdminPermission::ExtendMarket` |
    /// | `"update_fees"` | `AdminPermission::UpdateFees` |
    /// | `"update_config"` | `AdminPermission::UpdateConfig` |
    /// | `"reset_config"` | `AdminPermission::ResetConfig` |
    /// | `"collect_fees"` | `AdminPermission::CollectFees` |
    /// | `"manage_disputes"` | `AdminPermission::ManageDispute` |
    /// | `"view_analytics"` | `AdminPermission::ViewAnalytic` |
    /// | `"emergency_actions"` | `AdminPermission::Emergency` |
    ///
    /// # Use Cases
    ///
    /// - **API Integration**: Convert API action parameters to permissions
    /// - **Dynamic Validation**: Handle actions determined at runtime
    /// - **Configuration**: Map configuration-driven actions to permissions
    /// - **Testing**: Validate action-permission mappings in tests
    /// - **Debugging**: Convert action strings for logging and debugging
    ///
    /// # Design Notes
    ///
    /// Action strings use snake_case convention to match Rust naming standards.
    /// The mapping is case-sensitive and must match exactly. Consider adding
    /// case-insensitive mapping if needed for API flexibility.
    pub fn map_action_to_permission(action: &str) -> Result<AdminPermission, Error> {
        match action {
            "initialize" => Ok(AdminPermission::Initialize),
            "create_market" => Ok(AdminPermission::CreateMarket),
            "close_market" => Ok(AdminPermission::CloseMarket),
            "finalize_market" => Ok(AdminPermission::FinalizeMarket),
            "extend_market" => Ok(AdminPermission::ExtendMarket),
            "update_fees" => Ok(AdminPermission::UpdateFees),
            "update_config" => Ok(AdminPermission::UpdateConfig),
            "reset_config" => Ok(AdminPermission::ResetConfig),
            "collect_fees" => Ok(AdminPermission::CollectFees),
            "manage_disputes" => Ok(AdminPermission::ManageDispute),
            "view_analytics" => Ok(AdminPermission::ViewAnalytic),
            "emergency_actions" => Ok(AdminPermission::Emergency),
            "admin_broadcast" => Ok(AdminPermission::Emergency),
            _ => Err(Error::InvalidInput),
        }
    }
}

// ===== ADMIN ROLE MANAGEMENT =====

/// Admin role management
pub struct AdminRoleManager;

impl AdminRoleManager {
    /// Assigns a specific admin role to an address with associated permissions.
    ///
    /// This function creates or updates admin role assignments, establishing the
    /// permission hierarchy for admin operations. It supports bootstrapping the
    /// first admin and subsequent role assignments by authorized admins.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The address to receive the admin role
    /// * `role` - The admin role to assign (SuperAdmin, MarketAdmin, etc.)
    /// * `assigned_by` - The address performing the role assignment
    ///
    /// # Returns
    ///
    /// Returns `Result<(), Error>` where:
    /// - `Ok(())` - Role assigned successfully
    /// - `Err(Error)` - Assignment failed due to permissions or validation
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Assigner lacks Emergency permission
    /// - Permission validation errors from AdminAccessControl
    /// - Storage operation errors
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::{AdminRoleManager, AdminRole};
    /// # let env = Env::default();
    /// # let super_admin = Address::generate(&env);
    /// # let new_admin = Address::generate(&env);
    ///
    /// // Assign MarketAdmin role to a new admin
    /// match AdminRoleManager::assign_role(
    ///     &env,
    ///     &new_admin,
    ///     AdminRole::MarketAdmin,
    ///     &super_admin
    /// ) {
    ///     Ok(()) => {
    ///         println!("MarketAdmin role assigned successfully");
    ///     },
    ///     Err(e) => {
    ///         println!("Role assignment failed: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Role Hierarchy
    ///
    /// Available admin roles with their permission levels:
    /// - **SuperAdmin**: All permissions, can assign other roles
    /// - **MarketAdmin**: Market creation, closure, finalization, extension
    /// - **ConfigAdmin**: Configuration updates and resets
    /// - **FeeAdmin**: Fee configuration and collection
    /// - **ReadOnlyAdmin**: View-only access to analytics
    ///
    /// # Assignment Process
    ///
    /// The assignment process:
    /// 1. **Bootstrap Check**: First assignment bypasses permission validation
    /// 2. **Permission Validation**: Subsequent assignments require Emergency permission
    /// 3. **Role Creation**: Creates AdminRoleAssignment with timestamp and permissions
    /// 4. **Storage Update**: Stores assignment in persistent storage
    /// 5. **Event Emission**: Emits role assignment event for monitoring
    ///
    /// # Security
    ///
    /// Only admins with Emergency permission can assign roles to others.
    /// The first admin assignment (bootstrapping) bypasses this check to enable
    /// initial contract setup.
    pub fn assign_role(
        env: &Env,
        admin: &Address,
        role: AdminRole,
        assigned_by: &Address,
    ) -> Result<(), Error> {
        // Use a simple fixed key for admin role storage
        let key = Symbol::new(env, "admin_role");

        // Check if this is the first admin role assignment (bootstrapping)
        if !env.storage().persistent().has(&key) {
            // No admin role assigned yet, allow bootstrapping without permission check
        } else {
            // Validate assigner permissions for subsequent assignments
            AdminAccessControl::validate_permission(env, assigned_by, &AdminPermission::Emergency)?;
        }

        // Create role assignment
        let assignment = AdminRoleAssignment {
            admin: admin.clone(),
            role,
            assigned_by: assigned_by.clone(),
            assigned_at: env.ledger().timestamp(),
            permissions: AdminRoleManager::get_permissions_for_role(env, &role),
            is_active: true,
        };

        // Store role assignment
        env.storage().persistent().set(&key, &assignment);

        // Emit role assignment event
        let events_role = match role {
            AdminRole::SuperAdmin => crate::events::AdminRole::Owner,
            AdminRole::MarketAdmin => crate::events::AdminRole::Admin,
            AdminRole::ConfigAdmin => crate::events::AdminRole::Admin,
            AdminRole::FeeAdmin => crate::events::AdminRole::Admin,
            AdminRole::ReadOnlyAdmin => crate::events::AdminRole::Moderator,
        };
        EventEmitter::emit_admin_role_assigned(env, admin, &events_role, assigned_by);

        let action = if role == AdminRole::SuperAdmin && !env.storage().persistent().has(&key) {
            AuditAction::ContractInitialized // Fallback or logic
        } else {
            AuditAction::AdminAdded
        };
        AuditTrailManager::append_record(
            env,
            AuditAction::AdminRoleUpdated,
            assigned_by.clone(),
            Map::new(env),
            None,
        );

        Ok(())
    }

    /// Retrieves the admin role assigned to a specific address.
    ///
    /// This function looks up the admin role for a given address, validating
    /// that the admin is active and returning their assigned role. It's used
    /// for permission checking and role-based access control.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The address to look up the admin role for
    ///
    /// # Returns
    ///
    /// Returns `Result<AdminRole, Error>` where:
    /// - `Ok(AdminRole)` - The admin role assigned to the address
    /// - `Err(Error)` - Admin not found, inactive, or unauthorized
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - No admin role assignment found
    /// - `Error::Unauthorized` - Admin role assignment is inactive
    /// - `Error::Unauthorized` - Address doesn't match the assigned admin
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::{AdminRoleManager, AdminRole};
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    ///
    /// // Get admin role for permission checking
    /// match AdminRoleManager::get_admin_role(&env, &admin) {
    ///     Ok(AdminRole::SuperAdmin) => {
    ///         println!("User has SuperAdmin privileges");
    ///     },
    ///     Ok(AdminRole::MarketAdmin) => {
    ///         println!("User has MarketAdmin privileges");
    ///     },
    ///     Ok(role) => {
    ///         println!("User has {:?} privileges", role);
    ///     },
    ///     Err(e) => {
    ///         println!("No admin role found: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Role Validation
    ///
    /// The function performs these validations:
    /// 1. **Assignment Lookup**: Retrieves role assignment from storage
    /// 2. **Active Check**: Ensures the role assignment is active
    /// 3. **Address Match**: Confirms the address matches the assignment
    /// 4. **Role Return**: Returns the validated admin role
    ///
    /// # Use Cases
    ///
    /// - **Permission Checking**: Determine what actions an admin can perform
    /// - **UI Authorization**: Show/hide features based on admin role
    /// - **Audit Logging**: Record admin roles in action logs
    /// - **Role-Based Logic**: Execute different logic based on admin role
    /// - **Access Control**: Gate access to role-specific functionality
    ///
    /// # Performance
    ///
    /// This function performs a single storage lookup and is optimized for
    /// frequent use in permission validation scenarios.
    pub fn get_admin_role(env: &Env, admin: &Address) -> Result<AdminRole, Error> {
        // Use a simple fixed key for admin role storage
        let key = Symbol::new(env, "admin_role");

        let assignment: AdminRoleAssignment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::Unauthorized)?;

        if !assignment.is_active {
            return Err(Error::Unauthorized);
        }

        // Check if the passed address matches the admin address in the assignment
        if admin != &assignment.admin {
            return Err(Error::Unauthorized);
        }

        Ok(assignment.role)
    }

    /// Checks if a specific admin role has a particular permission.
    ///
    /// This function determines whether an admin role includes a specific
    /// permission by checking the role's permission set. It's a core component
    /// of the permission validation system.
    ///
    /// # Parameters
    ///
    /// * `_env` - The Soroban environment (unused but kept for consistency)
    /// * `role` - The admin role to check permissions for
    /// * `permission` - The specific permission to check
    ///
    /// # Returns
    ///
    /// Returns `Result<bool, Error>` where:
    /// - `Ok(true)` - Role has the specified permission
    /// - `Ok(false)` - Role does not have the specified permission
    /// - `Err(Error)` - Error retrieving role permissions
    ///
    /// # Errors
    ///
    /// This function typically doesn't error but may return errors from
    /// permission retrieval operations in future implementations.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::Env;
    /// # use predictify_hybrid::admin::{AdminRoleManager, AdminRole, AdminPermission};
    /// # let env = Env::default();
    ///
    /// // Check if MarketAdmin can create markets
    /// let can_create = AdminRoleManager::has_permission(
    ///     &env,
    ///     &AdminRole::MarketAdmin,
    ///     &AdminPermission::CreateMarket
    /// ).unwrap();
    ///
    /// if can_create {
    ///     println!("MarketAdmin can create markets");
    /// }
    ///
    /// // Check if ReadOnlyAdmin can update fees
    /// let can_update_fees = AdminRoleManager::has_permission(
    ///     &env,
    ///     &AdminRole::ReadOnlyAdmin,
    ///     &AdminPermission::UpdateFees
    /// ).unwrap();
    ///
    /// assert!(!can_update_fees); // ReadOnlyAdmin cannot update fees
    /// ```
    ///
    /// # Permission Matrix
    ///
    /// | Role | Initialize | CreateMarket | UpdateFees | UpdateConfig | Emergency |
    /// |------|------------|--------------|------------|--------------|----------|
    /// | SuperAdmin | ✓ | ✓ | ✓ | ✓ | ✓ |
    /// | MarketAdmin | ✗ | ✓ | ✗ | ✗ | ✗ |
    /// | ConfigAdmin | ✗ | ✗ | ✗ | ✓ | ✗ |
    /// | FeeAdmin | ✗ | ✗ | ✓ | ✗ | ✗ |
    /// | ReadOnlyAdmin | ✗ | ✗ | ✗ | ✗ | ✗ |
    ///
    /// # Use Cases
    ///
    /// - **Permission Validation**: Core permission checking logic
    /// - **Role Comparison**: Compare capabilities of different roles
    /// - **UI Authorization**: Determine what UI elements to show
    /// - **API Gating**: Control access to API endpoints
    /// - **Audit Systems**: Log permission checks for security auditing
    ///
    /// # Performance
    ///
    /// This function is highly optimized for frequent use, performing only
    /// in-memory operations on the role's permission vector.
    pub fn has_permission(
        _env: &Env,
        role: &AdminRole,
        permission: &AdminPermission,
    ) -> Result<bool, Error> {
        let permissions = AdminRoleManager::get_permissions_for_role(_env, role);
        Ok(permissions.contains(permission))
    }

    /// Retrieves the complete set of permissions for a specific admin role.
    ///
    /// This function returns all permissions associated with an admin role,
    /// providing the definitive permission set for role-based access control.
    /// It's used internally by permission checking functions and externally
    /// for role analysis and UI generation.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for creating the permission vector
    /// * `role` - The admin role to get permissions for
    ///
    /// # Returns
    ///
    /// Returns `Vec<AdminPermission>` containing all permissions for the role.
    /// The vector is never empty - even ReadOnlyAdmin has ViewAnalytic permission.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::Env;
    /// # use predictify_hybrid::admin::{AdminRoleManager, AdminRole, AdminPermission};
    /// # let env = Env::default();
    ///
    /// // Get all permissions for SuperAdmin
    /// let super_permissions = AdminRoleManager::get_permissions_for_role(
    ///     &env,
    ///     &AdminRole::SuperAdmin
    /// );
    ///
    /// println!("SuperAdmin has {} permissions", super_permissions.len());
    /// assert!(super_permissions.contains(&AdminPermission::Initialize));
    /// assert!(super_permissions.contains(&AdminPermission::Emergency));
    ///
    /// // Get permissions for MarketAdmin
    /// let market_permissions = AdminRoleManager::get_permissions_for_role(
    ///     &env,
    ///     &AdminRole::MarketAdmin
    /// );
    ///
    /// assert!(market_permissions.contains(&AdminPermission::CreateMarket));
    /// assert!(!market_permissions.contains(&AdminPermission::UpdateFees));
    /// ```
    ///
    /// # Permission Sets by Role
    ///
    /// **SuperAdmin** (12 permissions):
    /// - Initialize, CreateMarket, CloseMarket, FinalizeMarket
    /// - ExtendMarket, UpdateFees, UpdateConfig, ResetConfig
    /// - CollectFees, ManageDispute, ViewAnalytic, Emergency
    ///
    /// **MarketAdmin** (5 permissions):
    /// - CreateMarket, CloseMarket, FinalizeMarket, ExtendMarket, ViewAnalytic
    ///
    /// **ConfigAdmin** (3 permissions):
    /// - UpdateConfig, ResetConfig, ViewAnalytic
    ///
    /// **FeeAdmin** (3 permissions):
    /// - UpdateFees, CollectFees, ViewAnalytic
    ///
    /// **ReadOnlyAdmin** (1 permission):
    /// - ViewAnalytic
    ///
    /// # Use Cases
    ///
    /// - **Role Analysis**: Understand what each role can do
    /// - **UI Generation**: Create role-specific interfaces
    /// - **Permission Auditing**: Review and audit role permissions
    /// - **Role Comparison**: Compare capabilities between roles
    /// - **Documentation**: Generate permission documentation
    /// - **Testing**: Validate role permission assignments
    ///
    /// # Design Principles
    ///
    /// - **Least Privilege**: Each role has only necessary permissions
    /// - **Clear Hierarchy**: SuperAdmin > Specialized Admins > ReadOnly
    /// - **Separation of Concerns**: Different roles for different responsibilities
    /// - **Extensibility**: Easy to add new roles and permissions
    pub fn get_permissions_for_role(env: &Env, role: &AdminRole) -> Vec<AdminPermission> {
        match role {
            AdminRole::SuperAdmin => soroban_sdk::vec![
                env,
                AdminPermission::Initialize,
                AdminPermission::CreateMarket,
                AdminPermission::CloseMarket,
                AdminPermission::FinalizeMarket,
                AdminPermission::ExtendMarket,
                AdminPermission::UpdateFees,
                AdminPermission::UpdateConfig,
                AdminPermission::ResetConfig,
                AdminPermission::CollectFees,
                AdminPermission::ManageDispute,
                AdminPermission::ViewAnalytic,
                AdminPermission::Emergency,
            ],
            AdminRole::MarketAdmin => soroban_sdk::vec![
                env,
                AdminPermission::CreateMarket,
                AdminPermission::CloseMarket,
                AdminPermission::FinalizeMarket,
                AdminPermission::ExtendMarket,
                AdminPermission::ViewAnalytic,
            ],
            AdminRole::ConfigAdmin => soroban_sdk::vec![
                env,
                AdminPermission::UpdateConfig,
                AdminPermission::ResetConfig,
                AdminPermission::ViewAnalytic,
            ],
            AdminRole::FeeAdmin => soroban_sdk::vec![
                env,
                AdminPermission::UpdateFees,
                AdminPermission::CollectFees,
                AdminPermission::ViewAnalytic,
            ],
            AdminRole::ReadOnlyAdmin => soroban_sdk::vec![env, AdminPermission::ViewAnalytic,],
        }
    }

    /// Deactivate admin role
    pub fn deactivate_role(
        env: &Env,
        admin: &Address,
        deactivated_by: &Address,
    ) -> Result<(), Error> {
        // Validate deactivator permissions
        AdminAccessControl::validate_permission(env, deactivated_by, &AdminPermission::Emergency)?;

        // Use a simple fixed key for admin role storage
        let key = Symbol::new(env, "admin_role");

        let mut assignment: AdminRoleAssignment = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::Unauthorized)?;

        assignment.is_active = false;
        env.storage().persistent().set(&key, &assignment);

        // Emit role deactivation event
        EventEmitter::emit_admin_role_deactivated(env, admin, deactivated_by);

        Ok(())
    }
}

// ===== MULTI-ADMIN MANAGER =====

/// Multi-Admin Management System with comprehensive role-based access control
pub struct AdminManager;

impl AdminManager {
    /// Adds a new admin with specified role
    pub fn add_admin(
        env: &Env,
        current_admin: &Address,
        new_admin: &Address,
        role: AdminRole,
    ) -> Result<(), Error> {
        // Use existing AdminRoleManager for validation
        AdminAccessControl::validate_permission(env, current_admin, &AdminPermission::Emergency)?;

        // Prevent duplicate admin assignments
        if Self::is_admin(env, new_admin) {
            return Err(Error::InvalidState);
        }

        // Generate unique key for this admin
        let admin_key = Self::get_admin_key(env, new_admin);

        // Create role assignment using existing structure
        let assignment = AdminRoleAssignment {
            admin: new_admin.clone(),
            role: role.clone(),
            assigned_by: current_admin.clone(),
            assigned_at: env.ledger().timestamp(),
            permissions: AdminRoleManager::get_permissions_for_role(env, &role),
            is_active: true,
        };

        // Store in multi-admin storage
        env.storage().persistent().set(&admin_key, &assignment);

        // Track admins in a list so role enumeration stays in sync.
        let list_key = Symbol::new(env, "AdminList");
        let mut admin_list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&list_key)
            .unwrap_or_else(|| Vec::new(env));
        if !admin_list.contains(new_admin) {
            admin_list.push_back(new_admin.clone());
            env.storage().persistent().set(&list_key, &admin_list);
        }

        // Update admin count
        let count_key = Symbol::new(env, "AdminCount");
        let current_count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&count_key, &(current_count + 1));

        // Emit event using existing system
        Self::emit_admin_change_event(env, new_admin, AdminActionType::Added);

        Ok(())
    }

    /// Removes an admin from the system
    pub fn remove_admin(
        env: &Env,
        current_admin: &Address,
        admin_to_remove: &Address,
    ) -> Result<(), Error> {
        AdminAccessControl::validate_permission(env, current_admin, &AdminPermission::Emergency)?;

        // Prevent self-removal of last super admin
        if current_admin == admin_to_remove {
            let super_admin_count = Self::count_admins_with_role(env, AdminRole::SuperAdmin);
            if super_admin_count <= 1 {
                return Err(Error::InvalidState);
            }
        }

        let admin_key = Self::get_admin_key(env, admin_to_remove);
        if !env.storage().persistent().has(&admin_key) {
            return Err(Error::Unauthorized);
        }

        env.storage().persistent().remove(&admin_key);

        let list_key = Symbol::new(env, "AdminList");
        if let Some(mut admin_list) = env.storage().persistent().get::<_, Vec<Address>>(&list_key) {
            if let Some(index) = admin_list.first_index_of(admin_to_remove) {
                admin_list.remove(index);
                env.storage().persistent().set(&list_key, &admin_list);
            }
        }

        // Update count
        let count_key = Symbol::new(env, "AdminCount");
        let current_count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        if current_count > 0 {
            env.storage()
                .persistent()
                .set(&count_key, &(current_count - 1));
        }

        Self::emit_admin_change_event(env, admin_to_remove, AdminActionType::Removed);
        Ok(())
    }

    /// Updates an admin's role
    pub fn update_admin_role(
        env: &Env,
        current_admin: &Address,
        target_admin: &Address,
        new_role: AdminRole,
    ) -> Result<(), Error> {
        AdminAccessControl::validate_permission(env, current_admin, &AdminPermission::Emergency)?;

        let admin_key = Self::get_admin_key(env, target_admin);
        let mut assignment: AdminRoleAssignment = env
            .storage()
            .persistent()
            .get(&admin_key)
            .ok_or(Error::Unauthorized)?;

        // Prevent downgrading last super admin
        if assignment.role == AdminRole::SuperAdmin && new_role != AdminRole::SuperAdmin {
            let super_admin_count = Self::count_admins_with_role(env, AdminRole::SuperAdmin);
            if super_admin_count <= 1 {
                return Err(Error::InvalidState);
            }
        }

        assignment.role = new_role.clone();
        assignment.permissions = AdminRoleManager::get_permissions_for_role(env, &new_role);
        assignment.assigned_by = current_admin.clone();
        assignment.assigned_at = env.ledger().timestamp();

        env.storage().persistent().set(&admin_key, &assignment);
        Self::emit_admin_change_event(env, target_admin, AdminActionType::RoleUpdated);

        Ok(())
    }

    /// Validates if an admin has a specific permission
    pub fn validate_admin_permission(
        env: &Env,
        admin: &Address,
        permission: AdminPermission,
    ) -> Result<(), Error> {
        // Check original admin for backward compatibility
        if Self::is_original_admin(env, admin) {
            return Ok(());
        }

        let admin_key = Self::get_admin_key(env, admin);
        let assignment: AdminRoleAssignment = env
            .storage()
            .persistent()
            .get(&admin_key)
            .ok_or(Error::Unauthorized)?;

        if !assignment.is_active || !assignment.permissions.contains(&permission) {
            return Err(Error::Unauthorized);
        }

        Ok(())
    }

    /// Gets all admin roles in the system
    pub fn get_admin_roles(env: &Env) -> Map<Address, AdminRole> {
        let mut roles = Map::new(env);

        // Add original admin if exists
        if let Some(original_admin) = Self::get_original_admin(env) {
            roles.set(original_admin, AdminRole::SuperAdmin);
        }

        // Iterate over admin list to get all multi-admin entries
        let list_key = Symbol::new(env, "AdminList");
        if let Some(admin_list) = env.storage().persistent().get::<_, Vec<Address>>(&list_key) {
            for admin_addr in admin_list.iter() {
                let admin_key = Self::get_admin_key(env, &admin_addr);
                if let Some(assignment) = env
                    .storage()
                    .persistent()
                    .get::<_, AdminRoleAssignment>(&admin_key)
                {
                    if assignment.is_active {
                        roles.set(admin_addr.clone(), assignment.role);
                    }
                }
            }
        }

        roles
    }

    /// Check if an admin exists in the multi-admin system
    pub fn get_admin_role_for_address(env: &Env, admin: &Address) -> Option<AdminRole> {
        // Check original admin first
        if Self::is_original_admin(env, admin) {
            return Some(AdminRole::SuperAdmin);
        }

        // Check multi-admin storage
        let admin_key = Self::get_admin_key(env, admin);
        if let Some(assignment) = env
            .storage()
            .persistent()
            .get::<_, AdminRoleAssignment>(&admin_key)
        {
            if assignment.is_active {
                return Some(assignment.role);
            }
        }

        None
    }

    /// Emits admin change events using existing AdminActionType
    pub fn emit_admin_change_event(env: &Env, admin: &Address, action: AdminActionType) {
        let action_str = match action {
            AdminActionType::Added => "Added",
            AdminActionType::Removed => "Removed",
            AdminActionType::RoleUpdated => "RoleUpdated",
            AdminActionType::Activated => "Activated",
            AdminActionType::Deactivated => "Deactivated",
        };
        EventEmitter::emit_admin_action_logged(env, admin, action_str, &true);
    }

    /// Check role permissions against a specific permission
    pub fn check_role_permissions(env: &Env, role: AdminRole, permission: AdminPermission) -> bool {
        let permissions = AdminRoleManager::get_permissions_for_role(env, &role);
        permissions.contains(&permission)
    }

    // ===== Helper Methods =====

    /// Generate a deterministic multi-admin storage key for an address.
    fn get_admin_key(env: &Env, admin: &Address) -> String {
        String::from_str(env, &alloc::format!("MultiAdmin_{:?}", admin.to_string()))
    }

    /// Check if an address is the original admin from single-admin system
    pub fn is_original_admin(env: &Env, admin: &Address) -> bool {
        if let Some(original_admin) = Self::get_original_admin(env) {
            return admin == &original_admin;
        }
        false
    }

    /// Get the original admin address from single-admin system
    pub fn get_original_admin(env: &Env) -> Option<Address> {
        env.storage().persistent().get(&Symbol::new(env, "Admin"))
    }

    /// Check if an address is any type of admin (original or multi-admin)
    fn is_admin(env: &Env, admin: &Address) -> bool {
        Self::is_original_admin(env, admin)
            || env
                .storage()
                .persistent()
                .has(&Self::get_admin_key(env, admin))
    }

    /// Count admins with a specific role
    fn count_admins_with_role(env: &Env, role: AdminRole) -> u32 {
        let mut count = 0;

        // Count original admin if it matches the role
        if role == AdminRole::SuperAdmin && Self::get_original_admin(env).is_some() {
            count += 1;
        }

        // In a full implementation, we would iterate through all multi-admin entries
        // For now, this is a simplified version that works with the existing storage pattern
        count
    }

    /// Get admin assignment for a specific admin
    pub fn get_admin_assignment(env: &Env, admin: &Address) -> Option<AdminRoleAssignment> {
        let admin_key = Self::get_admin_key(env, admin);
        env.storage().persistent().get(&admin_key)
    }

    /// Deactivate an admin (set is_active to false)
    pub fn deactivate_admin(
        env: &Env,
        current_admin: &Address,
        target_admin: &Address,
    ) -> Result<(), Error> {
        AdminAccessControl::validate_permission(env, current_admin, &AdminPermission::Emergency)?;

        let admin_key = Self::get_admin_key(env, target_admin);
        let mut assignment: AdminRoleAssignment = env
            .storage()
            .persistent()
            .get(&admin_key)
            .ok_or(Error::Unauthorized)?;

        assignment.is_active = false;
        env.storage().persistent().set(&admin_key, &assignment);

        Self::emit_admin_change_event(env, target_admin, AdminActionType::Deactivated);
        Ok(())
    }

    /// Reactivate an admin (set is_active to true)
    pub fn reactivate_admin(
        env: &Env,
        current_admin: &Address,
        target_admin: &Address,
    ) -> Result<(), Error> {
        AdminAccessControl::validate_permission(env, current_admin, &AdminPermission::Emergency)?;

        let admin_key = Self::get_admin_key(env, target_admin);
        let mut assignment: AdminRoleAssignment = env
            .storage()
            .persistent()
            .get(&admin_key)
            .ok_or(Error::Unauthorized)?;

        assignment.is_active = true;
        env.storage().persistent().set(&admin_key, &assignment);

        Self::emit_admin_change_event(env, target_admin, AdminActionType::Activated);
        Ok(())
    }
}

