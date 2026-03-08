use anchor_lang::prelude::*;

use crate::contexts::{InitializePlatformConfig, UpdatePlatformConfig};
use crate::events::PlatformConfigUpdated;

pub fn initialize_platform_config(
    ctx: Context<InitializePlatformConfig>,
    treasury_authority: Pubkey,
) -> Result<()> {
    let platform_config = &mut ctx.accounts.platform_config;
    platform_config.authority = ctx.accounts.authority.key();
    platform_config.treasury_authority = treasury_authority;
    platform_config.bump = ctx.bumps.platform_config;

    emit!(PlatformConfigUpdated {
        authority: platform_config.authority,
        treasury_authority,
    });

    Ok(())
}

pub fn update_platform_config(
    ctx: Context<UpdatePlatformConfig>,
    treasury_authority: Pubkey,
) -> Result<()> {
    let platform_config = &mut ctx.accounts.platform_config;
    platform_config.treasury_authority = treasury_authority;

    emit!(PlatformConfigUpdated {
        authority: platform_config.authority,
        treasury_authority,
    });

    Ok(())
}
