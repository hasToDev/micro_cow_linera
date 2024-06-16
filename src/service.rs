#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
mod constants;

use std::sync::{Arc, Mutex};
use async_graphql::{EmptySubscription, Schema};
use async_graphql_derive::Object;
use self::state::MicroCow;
use linera_sdk::{
    base::WithServiceAbi,
    views::{View, ViewStorageContext},
    Service, ServiceRuntime,
};
use linera_sdk::base::{Amount, Owner, Timestamp};
use linera_sdk::graphql::GraphQLMutationRoot;
use micro_cow_linera::{BuyNotif, CowData, CowOperation, SellNotif};
use crate::constants::WELL_FED;

#[derive(Clone)]
pub struct MicroCowService {
    state: Arc<MicroCow>,
    runtime: Arc<Mutex<ServiceRuntime<Self>>>,
}

linera_sdk::service!(MicroCowService);

impl WithServiceAbi for MicroCowService {
    type Abi = micro_cow_linera::MicroCowAbi;
}

impl Service for MicroCowService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = MicroCow::load(ViewStorageContext::from(runtime.key_value_store()))
            .await
            .expect("Failed to load state");
        MicroCowService {
            state: Arc::new(state),
            runtime: Arc::new(Mutex::new(runtime)),
        }
    }

    async fn handle_query(&self, _query: Self::Query) -> Self::QueryResponse {
        let schema = Schema::build(
            self.clone(),
            CowOperation::mutation_root(),
            EmptySubscription,
        ).finish();
        schema.execute(_query).await
    }
}

/// ------------------------------------------------------------------------------------------
#[Object]
impl MicroCowService {
    async fn root_check(&self) -> bool {
        self.state.app_data.get().is_root
    }
    async fn status_check(&self) -> bool {
        self.state.app_data.get().is_initialize
    }
    async fn get_owner(&self) -> Owner {
        let appdata = self.state.app_data.get();
        appdata.owner
    }
    async fn get_balance(&self) -> Amount {
        let appdata = self.state.app_data.get();
        appdata.balance
    }
    async fn get_all_buy_notifications(&self) -> Vec<BuyNotif> {
        let buy_notifications = self.state
            .buy_notification
            .elements()
            .await
            .unwrap_or_else(|_| {
                panic!("unable to read Buy notifications");
            });
        buy_notifications
    }
    async fn get_one_buy_notification(&self) -> Vec<BuyNotif> {
        let notification = self.state.buy_notification.read_front(1).await.unwrap();
        notification
    }
    async fn get_all_sell_notifications(&self) -> Vec<SellNotif> {
        let buy_notifications = self.state
            .sell_notification
            .elements()
            .await
            .unwrap_or_else(|_| {
                panic!("unable to read Sell notifications");
            });
        buy_notifications
    }
    async fn get_one_sell_notification(&self) -> Vec<SellNotif> {
        let notification = self.state.sell_notification.read_front(1).await.unwrap();
        notification
    }
    async fn get_my_cows(&self) -> Vec<CowData> {
        self.state.get_my_cows().await
    }
    async fn get_one_local_db_cow(&self, cow_key: String) -> Vec<CowData> {
        let mut cow_list = Vec::new();
        // check if key exist
        let is_exist = self.state.cow_db.contains_key(&cow_key).await.unwrap_or(false);
        if !is_exist {
            return cow_list;
        }
        // get CowData is key exist
        let data = self.state.cow_db.get(&cow_key).await
            .unwrap_or_else(|_| {
                panic!("unable to get Cow DB Result");
            }).unwrap_or_else(|| {
            panic!("unable to get Cow DB Option");
        });
        cow_list.push(data);
        cow_list
    }
    async fn is_cow_alive(&self, cow_name: String, system_time: Timestamp) -> bool {
        self.state.is_cow_alive_and_exist(cow_name.clone(), system_time).await
    }
    async fn get_cow_existence(&self, cow_name: String) -> bool {
        self.state.is_cow_exist_in_db_and_ownership(cow_name).await
    }
    async fn is_cow_underage(&self, cow_born_time: Timestamp, system_time: Timestamp) -> bool {
        self.state.is_cow_underage(cow_born_time, system_time).await
    }
    async fn get_cow_sell_value(&self, cow_name: String) -> Amount {
        // check if name exist
        let is_exist = self.state.cow_db.contains_key(&cow_name).await.unwrap_or(false);
        if !is_exist {
            return Amount::ZERO;
        }

        // get CowData from DB
        let cow = self.state.cow_db.get(&cow_name).await
            .unwrap_or_else(|_| {
                panic!("unable to get Cow DB Result");
            }).unwrap_or_else(|| {
            panic!("unable to get Cow DB Option");
        });
        // get sell value
        self.state.cow_sell_value(cow).await
    }
    async fn is_cow_still_full(&self, cow_name: String, system_time: Timestamp) -> bool {
        // check if name exist
        let is_exist = self.state.cow_db.contains_key(&cow_name).await.unwrap_or(false);
        if !is_exist {
            return true;
        }

        // get CowData from DB
        let mut cow = self.state.cow_db.get(&cow_name).await
            .unwrap_or_else(|_| {
                panic!("unable to get Cow DB Result");
            }).unwrap_or_else(|| {
            panic!("unable to get Cow DB Option");
        });

        // find out feeding distance
        let current_time = system_time.micros();
        let last_fed_time = cow.last_fed_time.micros();
        let feed_distance = current_time - last_fed_time;

        if feed_distance <= WELL_FED {
            return true;
        }
        false
    }
    async fn count_all_cow_in_local_db(&self) -> usize {
        let cow_keys = self.state.cow_db.indices().await
            .unwrap_or_else(|_| {
                panic!("unable to read cow db");
            });
        cow_keys.len()
    }
}