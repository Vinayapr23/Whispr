// // use arcis_imports::*;

// // #[encrypted]
// // mod circuits {
// //     use arcis_imports::*;

// //     pub struct SwapData {
// //         is_x: bool,
// //         amount: u64,
// //         min_output: u64,
// //     }

// //     pub struct SwapResult {
// //         deposit_amount: u64,
// //         withdraw_amount: u64,
// //         is_x: bool,
// //     }

// //     #[instruction]
// //     pub fn compute_swap(
// //         swap_ctxt: Enc<Shared, SwapData>,
// //         vault_x_amount: u64,
// //         vault_y_amount: u64,
// //         lp_supply: u64,
// //         fee: u16
// //     ) -> Enc<Shared, SwapResult> {
// //         let swap = swap_ctxt.to_arcis();

// //         // Implement constant product AMM formula: x * y = k
// //         let k = vault_x_amount * vault_y_amount;

// //         let (deposit_amount, withdraw_amount) = if swap.is_x {
// //             // Swapping X for Y
// //             let new_x = vault_x_amount + swap.amount;
// //             let new_y = k / new_x;
// //             let y_out = vault_y_amount - new_y;

// //             // Apply fee (fee is in basis points, so divide by 10000)
// //             let fee_amount = (y_out * fee as u64) / 10000;
// //             let final_y_out = y_out - fee_amount;

// //             (swap.amount, final_y_out)
// //         } else {
// //             // Swapping Y for X
// //             let new_y = vault_y_amount + swap.amount;
// //             let new_x = k / new_y;
// //             let x_out = vault_x_amount - new_x;

// //             // Apply fee
// //             let fee_amount = (x_out * fee as u64) / 10000;
// //             let final_x_out = x_out - fee_amount;

// //             (swap.amount, final_x_out)
// //         };

// //         // Validate slippage protection
// //         let slippage_ok = withdraw_amount >= swap.min_output;

// //         let result = SwapResult {
// //             deposit_amount: if slippage_ok { deposit_amount } else { 0 },
// //             withdraw_amount: if slippage_ok { withdraw_amount } else { 0 },
// //             is_x: swap.is_x,
// //         };

// //         swap_ctxt.owner.from_arcis(result)
// //     }
// // }

// // // use arcis_imports::*;

// // // #[encrypted]
// // // mod circuits {
// // //     use arcis_imports::*;

// // //     pub struct SwapResult {
// // //         deposit_amount: u64,
// // //         withdraw_amount: u64,
// // //         is_x: bool,
// // //     }

// // //     #[instruction]
// // //     pub fn compute_swap(
// // //         is_x_ctxt: Enc<Shared, u8>,          // bool as u8
// // //         amount_ctxt: Enc<Shared, u64>,        // swap amount
// // //         min_output_ctxt: Enc<Shared, u64>,    // minimum output
// // //         vault_x_amount: u64,
// // //         vault_y_amount: u64,
// // //         lp_supply: u64,
// // //         fee: u16
// // //     ) -> Enc<Shared, SwapResult> {
// // //         // Decrypt the encrypted inputs
// // //         let is_x_val = is_x_ctxt.to_arcis();
// // //         let amount = amount_ctxt.to_arcis();
// // //         let min_output = min_output_ctxt.to_arcis();

// // //         // Convert u8 to bool
// // //         let is_x = is_x_val != 0;

// // //         // Implement constant product AMM formula: x * y = k
// // //         let k = vault_x_amount * vault_y_amount;

// // //         let (deposit_amount, withdraw_amount) = if is_x {
// // //             // Swapping X for Y
// // //             let new_x = vault_x_amount + amount;
// // //             let new_y = k / new_x;
// // //             let y_out = vault_y_amount - new_y;

// // //             // Apply fee (fee is in basis points, so divide by 10000)
// // //             let fee_amount = (y_out * fee as u64) / 10000;
// // //             let final_y_out = y_out - fee_amount;

// // //             (amount, final_y_out)
// // //         } else {
// // //             // Swapping Y for X
// // //             let new_y = vault_y_amount + amount;
// // //             let new_x = k / new_y;
// // //             let x_out = vault_x_amount - new_x;

// // //             // Apply fee
// // //             let fee_amount = (x_out * fee as u64) / 10000;
// // //             let final_x_out = x_out - fee_amount;

// // //             (amount, final_x_out)
// // //         };

// // //         // Validate slippage protection
// // //         let slippage_ok = withdraw_amount >= min_output;

// // //         let result = SwapResult {
// // //             deposit_amount: if slippage_ok { deposit_amount } else { 0 },
// // //             withdraw_amount: if slippage_ok { withdraw_amount } else { 0 },
// // //             is_x: is_x,
// // //         };

// // //         // Use the owner from one of the encrypted contexts to create the result
// // //         is_x_ctxt.owner.from_arcis(result)
// // //     }
// // // }

use arcis_imports::*;

#[encrypted]
mod circuits {
    use arcis_imports::*;

    // Simple struct with just amount
    pub struct SwapAmount {
        amount: u64,
    }

    // Return struct with both amounts
#[derive(Debug, Clone)]
pub struct SwapResult {
    pub deposit_amount: u64,   
    pub withdraw_amount: u64,   
}

    #[instruction]
    pub fn compute_swap(
        swap_amount_ctxt: Enc<Shared, SwapAmount>,
        vault_x_amount: u64,
        vault_y_amount: u64,
        lp_supply: u64,
        fee: u16,
    ) -> Enc<Shared, SwapResult> {
        // Return revealed struct
        let swap_amount = swap_amount_ctxt.to_arcis();
        let amount = swap_amount.amount;
        
        // Implement constant product AMM formula: x * y = k
        let k = vault_x_amount * vault_y_amount;
     
        // Always swapping X for Y
        let new_x = vault_x_amount + amount;
        let new_y = k / new_x;
        let y_out = vault_y_amount - new_y;

        // Apply fee
        let fee_amount = (y_out * fee as u64) / 10000;
        let final_y_out = y_out - fee_amount;

        let result = SwapResult {
            deposit_amount: amount,
            withdraw_amount: final_y_out,
        };

        swap_amount_ctxt.owner.from_arcis(result)
    }
}
