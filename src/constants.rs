/// The channel name the application uses for cross-chain messages about new posts.
pub const MICRO_COW_CHANNEL: &[u8] = b"cow_micro_chain_channel";

/// ------------------------------------------------------------------------------------------
pub const USER_INITIAL_TOKEN: u128 = 10000;
pub const UNIX_MICROS_IN_1_HOURS: u64 = 3_600_000_000;
pub const UNIX_MICROS_IN_24_HOURS: u64 = 86_400_000_000;
pub const UNIX_MICROS_IN_3_DAYS: u64 = 259_200_000_000;

/// [Cow Price]
/// ------------------------------------------------------------------------------------------
pub const JERSEY_PRICE: u128 = 1000;
pub const LIMOUSIN_PRICE: u128 = 1000;
pub const HALLIKAR_PRICE: u128 = 1000;
pub const HEREFORD_PRICE: u128 = 5000;
pub const HOLSTEIN_PRICE: u128 = 15000;
pub const SIMMENTAL_PRICE: u128 = 15000;

/// [Cow Feeding Limit]
/// WELL_FED = 6 hours
/// ON_TIME_FED = 12 hours
/// LATE_FED = 18 hours
/// all unit is in Unix Micros
/// ------------------------------------------------------------------------------------------
pub const WELL_FED: u64 = 21_600_000_000;
pub const ON_TIME_FEED: u64 = 43_200_000_000;
pub const LATE_FEED: u64 = 64_800_000_000;

/// [Cow Feeding Reward]
/// Cow feeding stats multiplier, with 2 digit decimal precision.
/// For every feeding event, it will give you:
/// 0.5% rewards when ON_TIME -- 50 (0.5 x 100)
/// 0.25% rewards when LATE -- 25 (0.25 x 100)
/// 1% fines when FORGOT -- 100 (1 x 100)
/// 100% equivalent to 10_000
/// ------------------------------------------------------------------------------------------
pub const ON_TIME_REWARD: i128 = 50;
pub const LATE_REWARD: i128 = 25;
pub const FORGOT_PENALTY: i128 = 100;
pub const PRECISION_100_PERCENT: i128 = 10_000;