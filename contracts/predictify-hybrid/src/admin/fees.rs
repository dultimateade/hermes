#![allow(unused_imports)]

use super::*;

impl AdminFunctions {
    /// Updates the platform fee configuration (admin only).
    ///
    /// This function allows authorized admins to modify the fee structure
    /// used throughout the platform, including platform fees, creation fees,
    /// and other fee-related parameters. Changes take effect immediately.
    ///
    /// # Parameters
    ///
    /// * `env` - The Soroban environment for blockchain operations
    /// * `admin` - The admin address performing the update (must have UpdateFees permission)
    /// * `new_config` - The new fee configuration to apply
    ///
    /// # Returns
    ///
    /// Returns `Result<FeeConfig, Error>` where:
    /// - `Ok(FeeConfig)` - Updated fee configuration
    /// - `Err(Error)` - Update failed due to permissions or validation
    ///
    /// # Errors
    ///
    /// This function returns specific errors:
    /// - `Error::Unauthorized` - Admin lacks UpdateFees permission
    /// - `Error::InvalidInput` - Fee configuration contains invalid values
    /// - Fee validation errors from FeeManager
    /// - Storage operation errors
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, Address};
    /// # use predictify_hybrid::admin::AdminFunctions;
    /// # use predictify_hybrid::fees::FeeConfig;
    /// # let env = Env::default();
    /// # let admin = Address::generate(&env);
    /// # let new_config = FeeConfig {
    /// #     platform_fee_percentage: 250, // 2.5%
    /// #     creation_fee: 1000000,        // 1 XLM
    /// #     min_stake: 100000,           // 0.1 XLM
    /// # };
    ///
    /// // Update platform fees
    /// match AdminFunctions::update_fee_config(&env, &admin, &new_config) {
    ///     Ok(updated_config) => {
    ///         println!("Fees updated successfully");
    ///         println!("New platform fee: {}%", updated_config.platform_fee_percentage / 100);
    ///     },
    ///     Err(e) => {
    ///         println!("Failed to update fees: {:?}", e);
    ///     }
    /// }
    /// ```
    ///
    /// # Fee Configuration Parameters
    ///
    /// The FeeConfig struct typically includes:
    /// - **Platform Fee Percentage**: Fee taken from winning payouts (basis points)
    /// - **Creation Fee**: Fee required to create new markets
    /// - **Minimum Stake**: Minimum amount required for voting
    /// - **Maximum Fee Cap**: Upper limit on total fees
    ///
    /// # Update Process
    ///
    /// The update process:
    /// 1. **Permission Validation**: Ensures admin has UpdateFees permission
    /// 2. **Configuration Validation**: Validates new fee parameters
    /// 3. **Fee Update**: Uses FeeManager to apply new configuration
    /// 4. **Action Logging**: Records fee update for audit trail
    ///
    /// # Impact and Considerations
    ///
    /// Fee updates have immediate platform-wide effects:
    /// - New markets use updated creation fees
    /// - Existing market resolutions use updated platform fees
    /// - User interfaces should reflect new fee structure
    /// - Consider gradual rollout for major fee changes
    ///
    /// # Best Practices
    ///
    /// - Announce fee changes to the community in advance
    /// - Test fee changes on testnet before mainnet deployment
    /// - Monitor platform activity after fee changes
    /// - Keep fees competitive with similar platforms
    /// - Document rationale for fee changes
    pub fn update_fee_config(
        env: &Env,
        admin: &Address,
        new_config: &FeeConfig,
    ) -> Result<(), Error> {
        // Validate admin permissions
        AdminAccessControl::validate_admin_for_action(env, admin, "update_fees")?;

        // Queue fee configuration with governance time-lock
        FeeManager::update_fee_config(env, admin.clone(), new_config.clone())?;

        // Log admin action
        let mut params = Map::new(env);
        params.set(
            String::from_str(env, "platform_fee"),
            String::from_str(env, "platform_fee"),
        );
        params.set(
            String::from_str(env, "creation_fee"),
            String::from_str(env, "creation_fee"),
        );
        AdminActionLogger::log_action(env, admin, "update_fees", None, params, true, None)?;

        Ok(())
    }

    /// Apply a previously queued fee configuration update.
    ///
    /// Succeeds only when the ETA has been reached. Can be called by anyone
    /// once the timelock expires.
    pub fn apply_fee_update(env: &Env, admin: &Address) -> Result<(), Error> {
        FeeManager::apply_fee_update(env, admin.clone())?;
        Ok(())
    }



}
