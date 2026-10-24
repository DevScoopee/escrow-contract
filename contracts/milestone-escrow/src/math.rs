//! Math module providing unified ratio splitting and weight allocation
//! with consistent rounding policies and strict conservation of funds.

use crate::Error;
use soroban_sdk::{Env, Vec};

/// Scale for basis points (10,000 = 100%).
pub const BPS_SCALE: u32 = 10_000;

/// Rounding mode for basis points and ratio splitting.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Rounding {
    /// Always round down (floor) on the first leg. The remainder goes to the second leg.
    Floor,
    /// Round half-up on the first leg (standard banker/commercial rounding).
    HalfUp,
    /// Round to nearest.
    Nearest,
}

/// Splits `amount` into two legs according to `bps` (0..=10,000).
///
/// Guarantees:
/// 1. `first + second == amount` (strict conservation of value).
/// 2. Neither leg is negative.
/// 3. All intermediate arithmetic is checked against overflow.
pub fn split_bps(amount: i128, bps: u32, rounding: Rounding) -> Result<(i128, i128), Error> {
    if amount < 0 {
        return Err(Error::InvalidAmount);
    }
    if bps > BPS_SCALE {
        return Err(Error::InvalidRatio);
    }
    let scale = BPS_SCALE as i128;
    let scaled = amount
        .checked_mul(bps as i128)
        .ok_or(Error::InvalidAmount)?;

    let first = match rounding {
        Rounding::Floor => scaled / scale,
        Rounding::HalfUp | Rounding::Nearest => {
            let half = scale / 2;
            scaled.checked_add(half).ok_or(Error::InvalidAmount)? / scale
        }
    };

    if first > amount {
        return Err(Error::InvalidAmount);
    }
    let second = amount.checked_sub(first).ok_or(Error::InvalidAmount)?;
    Ok((first, second))
}

/// Pure ratio-split helper with half-up rounding on the first leg.
///
/// Guarantees that `first + second == total`.
pub fn split_ratio_nearest(
    total: i128,
    numerator: i128,
    denominator: i128,
) -> Result<(i128, i128), Error> {
    if total < 0 || numerator < 0 || denominator <= 0 || numerator > denominator {
        return Err(Error::InvalidRatio);
    }

    let scaled = total.checked_mul(numerator).ok_or(Error::InvalidAmount)?;
    let half = denominator / 2;
    let rounded = scaled.checked_add(half).ok_or(Error::InvalidAmount)? / denominator;

    if rounded > total {
        return Err(Error::InvalidAmount);
    }

    let second = total.checked_sub(rounded).ok_or(Error::InvalidAmount)?;
    Ok((rounded, second))
}

/// Distributes `total_amount` across `weights` using the largest-remainder method
/// (Hamilton-Hare method). Guarantees that the sum of output parts equals `total_amount`.
pub fn allocate_weights(
    env: &Env,
    total_amount: i128,
    weights: &Vec<i128>,
) -> Result<Vec<i128>, Error> {
    if total_amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    if weights.is_empty() {
        return Err(Error::InvalidRatio);
    }

    let mut weight_sum: i128 = 0;
    for w in weights.iter() {
        if w < 0 {
            return Err(Error::InvalidRatio);
        }
        weight_sum = weight_sum.checked_add(w).ok_or(Error::InvalidRatio)?;
    }
    if weight_sum <= 0 {
        return Err(Error::InvalidRatio);
    }

    let mut allocations: Vec<i128> = Vec::new(env);
    let mut remainders: Vec<i128> = Vec::new(env);
    let mut allocated_total: i128 = 0;

    for w in weights.iter() {
        let weighted = total_amount.checked_mul(w).ok_or(Error::InvalidAmount)?;
        let base = weighted / weight_sum;
        let rem = weighted % weight_sum;

        allocations.push_back(base);
        remainders.push_back(rem);
        allocated_total = allocated_total
            .checked_add(base)
            .ok_or(Error::InvalidAmount)?;
    }

    let remaining = total_amount
        .checked_sub(allocated_total)
        .ok_or(Error::InvalidAmount)?;

    for _ in 0..remaining {
        let mut best_index: u32 = 0;
        let mut best_remainder: i128 = i128::MIN;

        for (idx, rem) in remainders.iter().enumerate() {
            if rem > best_remainder {
                best_remainder = rem;
                best_index = idx as u32;
            }
        }

        let current = allocations.get(best_index).ok_or(Error::InvalidAmount)?;
        allocations.set(
            best_index,
            current.checked_add(1).ok_or(Error::InvalidAmount)?,
        );
        remainders.set(best_index, i128::MIN);
    }

    Ok(allocations)
}

/// Allocates platform fees across 3 shares using largest remainder.
pub fn allocate_platform_fee_shares(
    total_amount: i128,
    client_bps: u32,
    freelancer_bps: u32,
    treasury_bps: u32,
) -> Result<[i128; 3], Error> {
    if total_amount == i128::MIN {
        return Err(Error::ArithmeticOverflow);
    }
    if total_amount < 0 {
        return Err(Error::InvalidAmount);
    }

    let scale = BPS_SCALE as i128;
    let ratios = [
        client_bps as i128,
        freelancer_bps as i128,
        treasury_bps as i128,
    ];
    let mut amounts = [0_i128; 3];
    let mut remainders = [0_i128; 3];
    let mut allocated = 0_i128;

    for index in 0..3 {
        let weighted = total_amount
            .checked_mul(ratios[index])
            .ok_or(Error::ArithmeticOverflow)?;
        amounts[index] = weighted
            .checked_div(scale)
            .ok_or(Error::ArithmeticOverflow)?;
        remainders[index] = weighted
            .checked_rem(scale)
            .ok_or(Error::ArithmeticOverflow)?;
        allocated = allocated
            .checked_add(amounts[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }

    let mut remaining = total_amount
        .checked_sub(allocated)
        .ok_or(Error::ArithmeticOverflow)?;
    while remaining > 0 {
        let mut best = 0_usize;
        for index in 1..3 {
            if remainders[index] > remainders[best] {
                best = index;
            }
        }
        amounts[best] = amounts[best]
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        remainders[best] = i128::MIN;
        remaining = remaining.checked_sub(1).ok_or(Error::ArithmeticOverflow)?;
    }

    let distributed = amounts[0]
        .checked_add(amounts[1])
        .and_then(|v| v.checked_add(amounts[2]))
        .ok_or(Error::ArithmeticOverflow)?;
    if distributed != total_amount {
        return Err(Error::ArithmeticOverflow);
    }

    Ok(amounts)
}
