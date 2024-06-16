use std::str::FromStr;
use async_graphql::{Request, Response, scalar};
use async_graphql_derive::{SimpleObject};
use linera_sdk::base::{Amount, ChainId, ContractAbi, Owner, ServiceAbi, Timestamp};
use linera_sdk::graphql::GraphQLMutationRoot;
use serde::{Deserialize, Serialize};

pub struct MicroCowAbi;

impl ContractAbi for MicroCowAbi {
    type Operation = CowOperation;
    type Response = ();
}

impl ServiceAbi for MicroCowAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// [Operation]
/// ------------------------------------------------------------------------------------------
#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum CowOperation {
    Initialize,
    DeleteBuyNotification,
    DeleteSellNotification,
    Subscribe,
    BuryDeadCows,
    BuyCow {
        owner: Owner,
        cow_name: String,
        cow_id: String,
        cow_breed: CowBreed,
    },
    FeedCow {
        owner: Owner,
        cow_name: String,
    },
    SellCow {
        owner: Owner,
        cow_name: String,
        cow_born_time: Timestamp,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    // executed by Root chain
    BuyCow {
        owner: Owner,
        cow_buy_params: CowBuyParams,
    },
    FeedCow {
        owner: Owner,
        cow_data: CowData,
    },
    SellCow {
        owner: Owner,
        cow_name: String,
    },
    // executed by User chain
    BuySuccess {
        cow_data: CowData,
    },
    BuyFailure {
        cow_data: CowData,
        cow_buy_params: CowBuyParams,
    },
    SellSuccess {
        cow_name: String,
        cow_owner: Owner,
        payment: Amount,
    },
    SellFailure {
        cow_name: String,
        reason: String,
    },
    FeedSuccess {
        cow_data: CowData,
    },
    Subscribe,
}

/// [AccountData]
/// ------------------------------------------------------------------------------------------
#[derive(
    Debug,
    Clone,
    Deserialize,
    Eq,
    Ord,
    PartialOrd,
    PartialEq,
    Serialize,
    SimpleObject
)]
pub struct AccountData {
    pub owner: Owner,
    pub chain_id: ChainId,
    pub balance: Amount,
    pub is_root: bool,
    pub is_initialize: bool,
}

impl AccountData {
    pub fn new(_owner: Owner, _balance: Amount, _chain: ChainId, _is_root: bool) -> Self {
        Self {
            owner: _owner,
            chain_id: _chain,
            balance: _balance,
            is_root: true,
            is_initialize: true,
        }
    }
}

impl Default for AccountData {
    fn default() -> Self {
        Self {
            owner: Owner::from_str("29e2be0c5e28220e834039b2b1d614e59222c86e109030fcea74fd556ab0028c").unwrap(),
            chain_id: ChainId::from_str("e4854ab09513d0e0b62497a5e190a074ff161c6c39e4dfa07dc5e2c0ee73d284").unwrap(),
            balance: Amount::ZERO,
            is_root: false,
            is_initialize: false,
        }
    }
}

/// [MicroCowParameters]
/// ------------------------------------------------------------------------------------------
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MicroCowParameters {
    /// Root Chain ID for channel
    pub root_chain_id: ChainId,
}

/// [CowBreed]
/// ------------------------------------------------------------------------------------------
scalar!(CowBreed);
#[derive(Debug, Clone, Copy, Deserialize, Eq, Ord, PartialOrd, PartialEq, Serialize)]
pub enum CowBreed {
    Jersey,
    Limousin,
    Hallikar,
    Hereford,
    Holstein,
    Simmental,
}

/// [CowGender]
/// ------------------------------------------------------------------------------------------
scalar!(CowGender);
#[derive(Debug, Clone, Copy, Deserialize, Eq, Ord, PartialOrd, PartialEq, Serialize)]
pub enum CowGender {
    Male,
    Female,
}

/// [CowBuyParams]
/// ------------------------------------------------------------------------------------------
scalar!(CowBuyParams);
#[derive(Debug, Clone, Deserialize, Eq, Ord, PartialOrd, PartialEq, Serialize)]
pub struct CowBuyParams {
    pub id: String,
    pub name: String,
    pub breed: CowBreed,
    pub gender: CowGender,
    pub price: Amount,
}

impl CowBuyParams {
    pub fn new(_id: String, _name: String, _breed: CowBreed, _gender: CowGender, _price: Amount) -> Self {
        Self {
            id: _id,
            name: _name,
            breed: _breed,
            gender: _gender,
            price: _price,
        }
    }
}

/// [CowData]
/// ------------------------------------------------------------------------------------------
scalar!(CowData);
#[derive(Debug, Clone, Deserialize, Eq, Ord, PartialOrd, PartialEq, Serialize)]
pub struct CowData {
    pub id: String,
    pub name: String,
    pub breed: CowBreed,
    pub gender: CowGender,
    pub born_time: Timestamp,
    pub last_fed_time: Timestamp,
    pub feeding_stats: FeedingStats,
    pub owner: Owner,
}

impl CowData {
    pub fn new(
        _id: String,
        _name: String,
        _breed: CowBreed,
        _gender: CowGender,
        _born_time: Timestamp,
        _last_fed_time: Timestamp,
        _feeding_stats: FeedingStats,
        _owner: Owner,
    ) -> Self {
        Self {
            id: _id,
            name: _name,
            breed: _breed,
            gender: _gender,
            born_time: _born_time,
            last_fed_time: _last_fed_time,
            feeding_stats: _feeding_stats,
            owner: _owner,
        }
    }
}

/// [FeedingStats]
/// ------------------------------------------------------------------------------------------
scalar!(FeedingStats);
#[derive(Debug, Clone, Deserialize, Eq, Ord, PartialOrd, PartialEq, Serialize)]
pub struct FeedingStats {
    pub on_time: u64,
    pub late: u64,
    pub forgot: u64,
}

impl FeedingStats {
    pub fn new() -> Self { Self { on_time: 0, late: 0, forgot: 0 } }
}

/// [BuyNotif]
/// ------------------------------------------------------------------------------------------
#[derive(
    Debug,
    Clone,
    Deserialize,
    Eq,
    Ord,
    PartialOrd,
    PartialEq,
    Serialize,
    SimpleObject
)]
pub struct BuyNotif {
    pub cow_name: String,
    pub is_success: bool,
}

impl Default for BuyNotif {
    fn default() -> Self {
        Self {
            cow_name: String::from(""),
            is_success: false,
        }
    }
}

/// [SellNotif]
/// ------------------------------------------------------------------------------------------
#[derive(
    Debug,
    Clone,
    Deserialize,
    Eq,
    Ord,
    PartialOrd,
    PartialEq,
    Serialize,
    SimpleObject
)]
pub struct SellNotif {
    pub cow_name: String,
    pub is_success: bool,
    pub failure_reason: String,
}

impl Default for SellNotif {
    fn default() -> Self {
        Self {
            cow_name: String::from(""),
            is_success: false,
            failure_reason: String::from(""),
        }
    }
}