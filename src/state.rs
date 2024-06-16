use linera_sdk::base::{Amount, ChainId, Owner, Timestamp};
use linera_sdk::views::{linera_views, MapView, QueueView, RegisterView, RootView, ViewStorageContext};
use micro_cow_linera::{AccountData, BuyNotif, CowBreed, CowData, SellNotif};
use crate::constants::*;

#[derive(RootView, async_graphql::SimpleObject)]
#[view(context = "ViewStorageContext")]
pub struct MicroCow {
    pub app_data: RegisterView<AccountData>,
    pub cow_db: MapView<String, CowData>,
    pub cow_ownership: MapView<String, u8>,
    pub buy_notification: QueueView<BuyNotif>,
    pub sell_notification: QueueView<SellNotif>,
}

/// ------------------------------------------------------------------------------------------

impl MicroCow {
    pub async fn initialize(&mut self, owner: Owner, chain_id: ChainId, balance: Amount, is_root: bool) {
        let mut appdata = self.app_data.get().clone();
        if appdata.is_initialize {
            panic!("User {:?} has been initialized", owner.clone());
        }
        appdata = AccountData { owner, chain_id, balance, is_root, is_initialize: true };
        self.app_data.set(appdata);
    }

    pub async fn debit(&mut self, amount: Amount) {
        let mut appdata = self.app_data.get().clone();
        appdata.balance
            .try_sub_assign(amount)
            .unwrap_or_else(|_| {
                panic!("Insufficient balance for {:?} to debit {}", appdata.owner, amount);
            });
        self.app_data.set(appdata);
    }

    pub async fn credit(&mut self, amount: Amount) {
        let mut appdata = self.app_data.get().clone();
        appdata.balance.saturating_add_assign(amount);
        self.app_data.set(appdata);
    }

    pub async fn create_new_or_update_cow_data(&mut self, cow_name: String, cow_data: CowData) {
        self.cow_db
            .insert(&cow_name.clone(), cow_data)
            .unwrap_or_else(|_| {
                panic!("Failed to create new Cow DB for {:?}", cow_name);
            });
    }

    pub async fn update_cow_ownership(&mut self, cow_name: String) {
        self.cow_ownership
            .insert(&cow_name.clone(), 1)
            .unwrap_or_else(|_| {
                panic!("Failed to update Cow Ownership for {:?}", cow_name);
            });
    }

    pub async fn delete_buy_notification(&mut self) {
        let count = self.buy_notification.count();
        if count != 0 {
            self.buy_notification.delete_front();
        };
    }

    pub async fn delete_sell_notification(&mut self) {
        let count = self.sell_notification.count();
        if count != 0 {
            self.sell_notification.delete_front();
        };
    }

    pub async fn is_cow_alive_and_exist(&self, cow_name: String, system_time: Timestamp) -> bool {
        // check if cow name exist in DB
        if self.cow_db.contains_key(&cow_name).await.unwrap_or(false) {
            // check if cow last fed time is more than 24 hours
            let cow = self.cow_db.get(&cow_name).await
                .unwrap_or_else(|_| {
                    panic!("unable to get Cow DB Result");
                }).unwrap_or_else(|| {
                panic!("unable to get Cow DB Option");
            });
            let last_fed_after_24_hours = cow.last_fed_time.micros() + UNIX_MICROS_IN_24_HOURS;
            let current_time = system_time.micros();
            if current_time.gt(&last_fed_after_24_hours) {
                // cow last fed time is more than 24 hours
                // therefore it has died, and everyone can claim it.
                return false;
            }
            return true;
        }
        false
    }

    pub async fn is_cow_exist_in_db_and_ownership(&self, cow_name: String) -> bool {
        let cow_in_local_db = self.cow_db.contains_key(&cow_name)
            .await.unwrap_or(false);
        let cow_in_ownership = self.cow_ownership.contains_key(&cow_name)
            .await.unwrap_or(false);
        if cow_in_local_db && cow_in_ownership {
            return true;
        }
        false
    }

    pub fn get_cow_price(&self, breed: CowBreed) -> Amount {
        let cow_price = match breed {
            CowBreed::Jersey => JERSEY_PRICE,
            CowBreed::Limousin => LIMOUSIN_PRICE,
            CowBreed::Hallikar => HALLIKAR_PRICE,
            CowBreed::Hereford => HEREFORD_PRICE,
            CowBreed::Holstein => HOLSTEIN_PRICE,
            CowBreed::Simmental => SIMMENTAL_PRICE,
        };
        Amount::from_tokens(cow_price)
    }

    pub async fn is_cow_underage(&self, cow_born_time: Timestamp, system_time: Timestamp) -> bool {
        // cow can be sold if its age is 3 days at minimum
        let cow_age = system_time.micros() - cow_born_time.micros();
        if cow_age < UNIX_MICROS_IN_3_DAYS {
            return true;
        }
        false
    }

    pub async fn cow_sell_value(&self, cow: CowData) -> Amount {
        // get cow price based on their breed
        let cow_base_price = self.get_cow_price(cow.breed.clone());

        // get cow appraisal price
        let cow_price_appraisal = self.get_cow_appraisal_price(
            cow, cow_base_price,
        ).await;

        cow_price_appraisal
    }

    pub async fn get_my_cows(&self) -> Vec<CowData> {
        // read all keys in cow ownership
        let cow_owner = self.app_data.get().owner;
        let cow_names = self.cow_ownership.indices().await
            .unwrap_or_else(|_| {
                panic!("unable to read cow ownership");
            });
        // create new vector and fill with CowData using previously obtained ownership keys
        let mut cow_list = Vec::new();
        for name in cow_names.into_iter() {
            // check if name exist
            let is_exist = self.cow_db.contains_key(&name).await.unwrap_or(false);
            if is_exist {
                // retrieve data
                let c = self.cow_db.get(&name).await
                    .unwrap_or_else(|_| {
                        panic!("unable to get Cow DB Result");
                    }).unwrap_or_else(|| {
                    panic!("unable to get Cow DB Option");
                });
                // check for the correct owner
                if cow_owner.eq(&c.owner) {
                    cow_list.push(c);
                }
            }
        }
        cow_list
    }

    pub async fn get_cow_appraisal_price(&self, cow: CowData, cow_base_price: Amount) -> Amount {
        // calculate appraisal multiplier
        let on_time_rewards = (cow.feeding_stats.on_time as i128) * ON_TIME_REWARD;
        let late_rewards = (cow.feeding_stats.late as i128) * LATE_REWARD;
        let forgot_penalties = (cow.feeding_stats.forgot as i128) * FORGOT_PENALTY;
        let mut rewards_penalties_multiplier = on_time_rewards + late_rewards - forgot_penalties;

        // check if the multiplier is a reward or a penalty
        let mut is_reward = true;
        if rewards_penalties_multiplier < 0 {
            is_reward = false;
            rewards_penalties_multiplier = rewards_penalties_multiplier * -1;
        }

        // if it's a penalty, check if the penalty more than 100%
        // penalty must not exceed 100%
        if !is_reward && rewards_penalties_multiplier > PRECISION_100_PERCENT {
            rewards_penalties_multiplier = PRECISION_100_PERCENT;
        }

        // calculate the amount of reward or penalty
        let mut rewards_or_penalty = cow_base_price
            .saturating_mul(rewards_penalties_multiplier as u128);
        rewards_or_penalty = Amount::from_tokens(rewards_or_penalty
            .saturating_div(Amount::from_tokens(PRECISION_100_PERCENT as u128)));

        // calculate and return the appraisal price
        if !is_reward {
            return cow_base_price.saturating_sub(rewards_or_penalty);
        }
        cow_base_price.saturating_add(rewards_or_penalty)
    }
}