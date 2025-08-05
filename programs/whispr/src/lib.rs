use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::{spl_associated_token_account, AssociatedToken},
    token::{
        burn, mint_to, spl_token, transfer, Burn, Mint, MintTo, Token, TokenAccount, Transfer,
    },
};
use arcium_anchor::prelude::*;
use constant_product_curve::ConstantProduct;
use spl_associated_token_account::id as ASSOCIATED_TOKEN_PROGRAM_ID;
use spl_token::ID as TOKEN_PROGRAM_ID;

const COMP_DEF_OFFSET_COMPUTE_SWAP: u32 = comp_def_offset("compute_swap");

declare_id!("AmZXddBcEnTS6T4k8TxDsDx3R5wE16qji67Lwh192a3M");

#[arcium_program]
pub mod whispr {
    use arcium_client::idl::arcium::types::CallbackAccount;

    use super::*;

    // ========================= AMM FUNCTIONALITY =========================
    pub fn initialize_amm(
        ctx: Context<InitializeAmm>,
        seed: u64,
        fee: u16,
        authority: Option<Pubkey>,
    ) -> Result<()> {
        ctx.accounts.config.set_inner(Config {
            seed,
            authority,
            mint_x: ctx.accounts.mint_x.key(),
            mint_y: ctx.accounts.mint_y.key(),
            fee,
            locked: false,
            config_bump: ctx.bumps.config,
            lp_bump: ctx.bumps.mint_lp,
        });

        emit!(InitializeEvent {
            admin: ctx.accounts.admin.key(),
            mint_x: ctx.accounts.mint_x.key(),
            mint_y: ctx.accounts.mint_y.key(),
            mint_lp: ctx.accounts.mint_lp.key(),
            vault_x: ctx.accounts.vault_x.key(),
            vault_y: ctx.accounts.vault_y.key(),
            config: ctx.accounts.config.key(),
            fee,
        });
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64, max_x: u64, max_y: u64) -> Result<()> {
        require!(ctx.accounts.config.locked == false, ErrorCode::PoolLocked);
        require!(amount != 0, ErrorCode::InvalidAmount);

        let (x, y) = match ctx.accounts.mint_lp.supply == 0
            && ctx.accounts.vault_x.amount == 0
            && ctx.accounts.vault_y.amount == 0
        {
            true => (max_x, max_y),
            false => {
                let amounts = ConstantProduct::xy_deposit_amounts_from_l(
                    ctx.accounts.vault_x.amount,
                    ctx.accounts.vault_y.amount,
                    ctx.accounts.mint_lp.supply,
                    amount,
                    6,
                )
                .unwrap();
                (amounts.x, amounts.y)
            }
        };

        require!(x <= max_x && y <= max_y, ErrorCode::SlippageExceded);

        // Transfer tokens
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_x.to_account_info(),
                    to: ctx.accounts.vault_x.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            x,
        )?;

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_y.to_account_info(),
                    to: ctx.accounts.vault_y.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            y,
        )?;

        // Mint LP tokens
        let seeds = &[
            &b"config"[..],
            &ctx.accounts.config.seed.to_le_bytes(),
            &[ctx.accounts.config.config_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint_lp.to_account_info(),
                    to: ctx.accounts.user_lp.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            amount,
            x_amount: x,
            y_amount: y,
        });
        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64, min_x: u64, min_y: u64) -> Result<()> {
        require!(ctx.accounts.config.locked == false, ErrorCode::PoolLocked);
        require!(amount != 0, ErrorCode::InvalidAmount);

        let amounts = ConstantProduct::xy_withdraw_amounts_from_l(
            ctx.accounts.vault_x.amount,
            ctx.accounts.vault_y.amount,
            ctx.accounts.mint_lp.supply,
            amount,
            6,
        )
        .map_err(|_| ErrorCode::InvalidAmount)?;

        require!(
            amounts.x >= min_x && amounts.y >= min_y,
            ErrorCode::SlippageExceded
        );

        let seeds = &[
            &b"config"[..],
            &ctx.accounts.config.seed.to_le_bytes(),
            &[ctx.accounts.config.config_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        // Withdraw tokens
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_x.to_account_info(),
                    to: ctx.accounts.user_x.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer_seeds,
            ),
            amounts.x,
        )?;

        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_y.to_account_info(),
                    to: ctx.accounts.user_y.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer_seeds,
            ),
            amounts.y,
        )?;

        // Burn LP tokens
        burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.mint_lp.to_account_info(),
                    from: ctx.accounts.user_lp.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(WithdrawEvent {
            user: ctx.accounts.user.key(),
            amount,
            x_amount: amounts.x,
            y_amount: amounts.y,
        });
        Ok(())
    }

    pub fn lock(ctx: Context<Update>) -> Result<()> {
        require!(
            ctx.accounts.config.authority == Some(ctx.accounts.user.key()),
            ErrorCode::InvalidAuthority
        );
        ctx.accounts.config.locked = true;
        emit!(LockEvent {
            user: ctx.accounts.user.key(),
            config: ctx.accounts.config.key(),
        });
        Ok(())
    }

    pub fn unlock(ctx: Context<Update>) -> Result<()> {
        require!(
            ctx.accounts.config.authority == Some(ctx.accounts.user.key()),
            ErrorCode::InvalidAuthority
        );
        ctx.accounts.config.locked = false;
        emit!(UnlockEvent {
            user: ctx.accounts.user.key(),
            config: ctx.accounts.config.key(),
        });
        Ok(())
    }

    // ========================= CONFIDENTIAL SWAP =========================
    pub fn init_compute_swap_comp_def(ctx: Context<InitComputeSwapCompDef>) -> Result<()> {
        init_comp_def(ctx.accounts, true, 0, None, None)?;
        Ok(())
    }

    pub fn compute_swap(
        ctx: Context<ComputeSwap>,
        computation_offset: u64,
        pub_key: [u8; 32],
        nonce: u128,
        // encrypted_is_x: [u8; 32],      // Encrypted bool (as u8)
        encrypted_amount: [u8; 32], // Encrypted u64
                                    // encrypted_min_output: [u8; 32], // Encrypted u64
    ) -> Result<()> {
        require!(ctx.accounts.config.locked == false, ErrorCode::PoolLocked);

        // Initialize swap state
        let clock = Clock::get()?;
        ctx.accounts.swap_state.user = ctx.accounts.user.key();
        ctx.accounts.swap_state.config = ctx.accounts.config.key();
        ctx.accounts.swap_state.computation_offset = computation_offset;
        // ctx.accounts.swap_state.is_x = false;
        ctx.accounts.swap_state.amount = 0;
        ctx.accounts.swap_state.min_output = 0;
        ctx.accounts.swap_state.status = SwapStatus::Initiated;
        ctx.accounts.swap_state.created_at = clock.unix_timestamp;

        // Pass three encrypted values separately
        let args = vec![
            Argument::ArcisPubkey(pub_key),
            Argument::PlaintextU128(nonce),
            Argument::EncryptedU64(encrypted_amount), // amount
            //  Argument::EncryptedU64(encrypted_min_output), // min_output
            Argument::PlaintextU64(ctx.accounts.vault_x.amount),
            Argument::PlaintextU64(ctx.accounts.vault_y.amount),
            Argument::PlaintextU64(ctx.accounts.mint_lp.supply),
            Argument::PlaintextU16(ctx.accounts.config.fee),
        ];

        queue_computation(
            ctx.accounts,
            computation_offset,
            args,
            vec![
                CallbackAccount {
                    pubkey: ctx.accounts.mint_x.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.mint_y.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.mint_lp.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.config.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.swap_state.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.vault_x.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.vault_x.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.user_x.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.user_y.key(),
                    is_writable: true,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.token_program.key(),
                    is_writable: false,
                },
                CallbackAccount {
                    pubkey: ctx.accounts.associated_token_program.key(),
                    is_writable: false,
                },
            ],
            None,
        )?;

        ctx.accounts.swap_state.status = SwapStatus::Computing;

        emit!(ConfidentialSwapInitiatedEvent {
            user: ctx.accounts.user.key(),
            config: ctx.accounts.config.key(),
            computation_offset,
        });

        Ok(())
    }

    #[arcium_callback(encrypted_ix = "compute_swap")]
    pub fn compute_swap_callback(
        ctx: Context<ComputeSwapCallback>,
        output: ComputationOutputs<ComputeSwapOutput>,
    ) -> Result<()> {
        // Extract results from MPC computation
        let swap_result = match output {
            ComputationOutputs::Success(ComputeSwapOutput { field_0: o }) => o,
            _ => return Err(ErrorCode::AbortedComputation.into()),
        };

        // Decrypt the results
        // The circuit returns SwapResult with three fields
        let deposit_amount =
            u64::from_le_bytes(swap_result.ciphertexts[0][..8].try_into().unwrap());

        let withdraw_amount =
            u64::from_le_bytes(swap_result.ciphertexts[1][..8].try_into().unwrap());
        //let is_x = swap_result.ciphertexts[2][0] != 0;

        // Validate amounts
        require!(
            deposit_amount > 0 || withdraw_amount == 0,
            ErrorCode::InvalidAmount
        );

        // If withdraw_amount is 0, it means slippage check failed
        // if withdraw_amount == 0 {
        //     ctx.accounts.swap_state.status = SwapStatus::Failed;
        //     emit!(ConfidentialSwapFailedEvent {
        //         user: ctx.accounts.user.key(),
        //         config: ctx.accounts.config.key(),
        //         computation_offset: ctx.accounts.swap_state.computation_offset,
        //         reason: "Slippage protection triggered".to_string(),
        //     });
        //     return Ok(());
        // }

        // Execute token transfers
        let seeds = &[
            &b"config"[..],
            &ctx.accounts.config.seed.to_le_bytes(),
            &[ctx.accounts.config.config_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        // Deposit and withdraw tokens based on direction
        //if is_x {
        // User gives X, gets Y
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_x.to_account_info(),
                    to: ctx.accounts.vault_x.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            deposit_amount,
        )?;

        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_y.to_account_info(),
                    to: ctx.accounts.user_y.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer_seeds,
            ),
            withdraw_amount,
        )?;
        //   //  } else {
        //         // User gives Y, gets X
        //         transfer(
        //             CpiContext::new(
        //                 ctx.accounts.token_program.to_account_info(),
        //                 Transfer {
        //                     from: ctx.accounts.user_y.to_account_info(),
        //                     to: ctx.accounts.vault_y.to_account_info(),
        //                     authority: ctx.accounts.user.to_account_info(),
        //                 }
        //             ),
        //             deposit_amount
        //         )?;

        //         transfer(
        //             CpiContext::new_with_signer(
        //                 ctx.accounts.token_program.to_account_info(),
        //                 Transfer {
        //                     from: ctx.accounts.vault_x.to_account_info(),
        //                     to: ctx.accounts.user_x.to_account_info(),
        //                     authority: ctx.accounts.config.to_account_info(),
        //                 },
        //                 signer_seeds
        //             ),
        //             withdraw_amount
        //         )?;
        //     }

        // Update swap state
        ctx.accounts.swap_state.status = SwapStatus::Executed;
        //ctx.accounts.swap_state.is_x = is_x;
        ctx.accounts.swap_state.amount = deposit_amount;
        ctx.accounts.swap_state.min_output = withdraw_amount;

        emit!(ConfidentialSwapExecutedEvent {
            user: ctx.accounts.user.key(),
            config: ctx.accounts.config.key(),
            computation_offset: ctx.accounts.swap_state.computation_offset,
            deposit_amount,
            withdraw_amount,
            // is_x,
        });

        Ok(())
    }
}

// ========================= STATE =========================

#[account]
pub struct Config {
    pub seed: u64,
    pub authority: Option<Pubkey>,
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    pub fee: u16,
    pub locked: bool,
    pub config_bump: u8,
    pub lp_bump: u8,
}

impl Space for Config {
    const INIT_SPACE: usize = 8 + 8 + 32 + 1 + 32 * 2 + 2 + 1 + 1 * 2;
}

#[account]
pub struct SwapState {
    pub user: Pubkey,
    pub config: Pubkey,
    pub computation_offset: u64,
    //  pub is_x: bool,
    pub amount: u64,
    pub min_output: u64,
    pub status: SwapStatus,
    pub created_at: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum SwapStatus {
    Initiated,
    Computing,
    Computed,
    Executed,
    Failed,
}

impl Space for SwapState {
    const INIT_SPACE: usize = 8 + 32 + 32 + 8  + 8 + 8 + 1 + 8;
}

// ========================= AMM ACCOUNTS =========================

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct InitializeAmm<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        init,
        payer = admin,
        seeds = [b"lp", config.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = config
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = admin,
        seeds = [b"config", seed.to_le_bytes().as_ref()],
        bump,
        space = Config::INIT_SPACE,
    )]
    pub config: Account<'info, Config>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
        has_one = mint_x,
        has_one = mint_y,
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
    )]
    pub user_lp: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
        has_one = mint_x,
        has_one = mint_y
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config
    )]
    pub vault_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config
    )]
    pub vault_y: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user
    )]
    pub user_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user
    )]
    pub user_y: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = mint_lp,
        associated_token::authority = user
    )]
    pub user_lp: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,
}

// ========================= CONFIDENTIAL SWAP ACCOUNTS =========================

#[queue_computation_accounts("compute_swap", user)]
#[derive(Accounts)]
#[instruction(computation_offset: u64)]
pub struct ComputeSwap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
        has_one = mint_x,
        has_one = mint_y,
    )]
    pub config: Box<Account<'info, Config>>,
    #[account(
        mut,
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Box<Account<'info, Mint>>,
    #[account(
        mut,
       associated_token::mint = mint_x,
       associated_token::authority = config,
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
       associated_token::mint = mint_y,
       associated_token::authority = config,
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = user,
        space = SwapState::INIT_SPACE,
        seeds = [b"swap_state"],
        bump
    )]
    pub swap_state: Box<Account<'info, SwapState>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,

    // Arcium required accounts
    #[account(address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut, address = derive_mempool_pda!())]
    /// CHECK: mempool_account, checked by the arcium program.
    pub mempool_account: UncheckedAccount<'info>,
    #[account(mut, address = derive_execpool_pda!())]
    /// CHECK: executing_pool, checked by the arcium program.
    pub executing_pool: UncheckedAccount<'info>,
    #[account(mut, address = derive_comp_pda!(computation_offset))]
    /// CHECK: computation_account, checked by the arcium program.
    pub computation_account: UncheckedAccount<'info>,
    #[account(address = derive_comp_def_pda!(COMP_DEF_OFFSET_COMPUTE_SWAP))]
    pub comp_def_account: Box<Account<'info, ComputationDefinitionAccount>>,
    #[account(mut, address = derive_cluster_pda!(mxe_account))]
    pub cluster_account: Box<Account<'info, Cluster>>,
    #[account(mut, address = ARCIUM_FEE_POOL_ACCOUNT_ADDRESS)]
    pub pool_account: Box<Account<'info, FeePool>>,
    #[account(address = ARCIUM_CLOCK_ACCOUNT_ADDRESS)]
    pub clock_account: Box<Account<'info, ClockAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub arcium_program: Program<'info, Arcium>,
}

#[callback_accounts("compute_swap", user)]
#[derive(Accounts)]
pub struct ComputeSwapCallback<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub comp_def_account: Box<Account<'info, ComputationDefinitionAccount>>,
    #[account(address = ::anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: instructions_sysvar, checked by the account constraint
    pub instructions_sysvar: AccountInfo<'info>,

    pub mint_x: Account<'info, Mint>,

    pub mint_y: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
        has_one = mint_x,
        has_one = mint_y,
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [b"swap_state"],
        bump
    )]
    pub swap_state: Account<'info, SwapState>,
    #[account(mut)]
    pub vault_x: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_y: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_x: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_y: Account<'info, TokenAccount>,
    #[account(address = derive_comp_def_pda!(COMP_DEF_OFFSET_COMPUTE_SWAP))]
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
// #[callback_accounts("compute_swap", user)]
// #[derive(Accounts)]
// pub struct ComputeSwapCallback<'info> {
//     #[account(mut)]
//     pub user: Signer<'info>,

//     // Use AccountInfo for problematic accounts in callbacks
//     /// CHECK: Mint X
//     pub mint_x: AccountInfo<'info>,
//     /// CHECK: Mint Y
//     pub mint_y: AccountInfo<'info>,
//     /// CHECK: LP Mint
//     #[account(mut)]
//     pub mint_lp: AccountInfo<'info>,

//     // Config can stay typed since it's your program's account
//     pub config: Account<'info, Config>,

//     #[account(mut)]
//     pub swap_state: Account<'info, SwapState>,

//     // Token accounts as AccountInfo
//     /// CHECK: Vault X
//     #[account(mut)]
//     pub vault_x: AccountInfo<'info>,
//     /// CHECK: Vault Y
//     #[account(mut)]
//     pub vault_y: AccountInfo<'info>,
//     /// CHECK: User X
//     #[account(mut)]
//     pub user_x: AccountInfo<'info>,
//     /// CHECK: User Y
//     #[account(mut)]
//     pub user_y: AccountInfo<'info>,

//     pub comp_def_account: Account<'info, ComputationDefinitionAccount>,

//     /// CHECK: Instructions sysvar
//     pub instructions_sysvar: AccountInfo<'info>,

//     pub token_program: Program<'info, Token>,
//      pub associated_token_program: Program<'info, AssociatedToken>,
//        pub system_program: Program<'info, System>,
//     pub arcium_program: Program<'info, Arcium>,

// }
#[init_computation_definition_accounts("compute_swap", payer)]
#[derive(Accounts)]
pub struct InitComputeSwapCompDef<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, address = derive_mxe_pda!())]
    pub mxe_account: Box<Account<'info, MXEAccount>>,
    #[account(mut)]
    /// CHECK: comp_def_account, checked by arcium program.
    pub comp_def_account: UncheckedAccount<'info>,
    pub arcium_program: Program<'info, Arcium>,
    pub system_program: Program<'info, System>,
}

// ========================= EVENTS =========================

#[event]
pub struct InitializeEvent {
    pub admin: Pubkey,
    pub mint_x: Pubkey,
    pub mint_y: Pubkey,
    pub mint_lp: Pubkey,
    pub vault_x: Pubkey,
    pub vault_y: Pubkey,
    pub config: Pubkey,
    pub fee: u16,
}

#[event]
pub struct DepositEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub x_amount: u64,
    pub y_amount: u64,
}

#[event]
pub struct WithdrawEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub x_amount: u64,
    pub y_amount: u64,
}

#[event]
pub struct LockEvent {
    pub user: Pubkey,
    pub config: Pubkey,
}

#[event]
pub struct UnlockEvent {
    pub user: Pubkey,
    pub config: Pubkey,
}

#[event]
pub struct ConfidentialSwapInitiatedEvent {
    pub user: Pubkey,
    pub config: Pubkey,
    pub computation_offset: u64,
}

#[event]
pub struct ConfidentialSwapExecutedEvent {
    pub user: Pubkey,
    pub config: Pubkey,
    pub computation_offset: u64,
    pub deposit_amount: u64,
    pub withdraw_amount: u64,
    // pub is_x: bool,
}

#[event]
pub struct ConfidentialSwapFailedEvent {
    pub user: Pubkey,
    pub config: Pubkey,
    pub computation_offset: u64,
    pub reason: String,
}

// ========================= ERRORS =========================

#[error_code]
pub enum ErrorCode {
    #[msg("The computation was aborted")]
    AbortedComputation,
    #[msg("Cluster not set")]
    ClusterNotSet,
    #[msg("This pool is locked")]
    PoolLocked,
    #[msg("Slippage exceeded")]
    SlippageExceded,
    #[msg("Invalid Amount")]
    InvalidAmount,
    #[msg("Invalid update authority")]
    InvalidAuthority,
}
