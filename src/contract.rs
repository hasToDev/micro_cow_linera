#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
mod random;
mod constants;

use linera_sdk::{
    base::WithContractAbi,
    views::{RootView, View, ViewStorageContext},
    Contract, ContractRuntime,
};
use linera_sdk::base::{Amount, ChannelName, Destination, Owner, Timestamp};
use micro_cow_linera::{BuyNotif, CowBreed, CowBuyParams, CowData, CowGender, CowOperation, FeedingStats, Message, MicroCowParameters, SellNotif};
use crate::constants::*;
use crate::random::{custom_getrandom, truncate};

use self::state::MicroCow;

pub struct MicroCowContract {
    state: MicroCow,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(MicroCowContract);

impl WithContractAbi for MicroCowContract {
    type Abi = micro_cow_linera::MicroCowAbi;
}

impl Contract for MicroCowContract {
    type Message = Message;
    type Parameters = MicroCowParameters;
    type InstantiationArgument = Amount;

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = MicroCow::load(ViewStorageContext::from(runtime.key_value_store()))
            .await
            .expect("Failed to load state");
        MicroCowContract { state, runtime }
    }

    async fn instantiate(&mut self, _argument: Self::InstantiationArgument) {
        log::info!("App Initialization");

        // initialization amount from argument
        let amount = _argument;

        // validate that the application parameters were configured correctly.
        let app_params = self.runtime.application_parameters();
        log::info!("Parameter ROOT Chain ID: {}", app_params.root_chain_id);


        if let Some(owner) = self.runtime.authenticated_signer() {
            let chain_id = self.runtime.chain_id();

            // make sure runtime Chain ID is equal with Root Chain ID from parameters
            assert_eq!(
                chain_id,
                app_params.root_chain_id,
                "runtime ChainID doesn't match ChainID parameters"
            );

            self.state.initialize(owner, chain_id, amount, true).await;
        }
    }

    async fn execute_operation(&mut self, _operation: Self::Operation) -> Self::Response {
        // root chain are not allowed to play
        self.check_root_invocation();

        match _operation {
            CowOperation::Initialize => {
                log::info!("CowOperation::Initialize");
                // initialize user account with 10_000 token
                let chain_id = self.runtime.chain_id();
                let owner = self.runtime.authenticated_signer().unwrap();
                self.subscribe_to_micro_cow_channel();
                self.state.initialize(owner, chain_id, Amount::from_tokens(USER_INITIAL_TOKEN), false).await;
            }
            CowOperation::DeleteBuyNotification => {
                log::info!("CowOperation::DeleteBuyNotification");
                self.state.delete_buy_notification().await
            }
            CowOperation::DeleteSellNotification => {
                log::info!("CowOperation::DeleteSellNotification");
                self.state.delete_sell_notification().await
            }
            CowOperation::Subscribe => {
                log::info!("CowOperation::Subscribe");
                self.subscribe_to_micro_cow_channel();
            }
            CowOperation::BuyCow { owner, cow_name, cow_id, cow_breed } => {
                log::info!("CowOperation::BuyCow");
                // check authentication
                self.check_authentication(owner);

                // check cow name in DB and available to buy
                // you can't buy if cow exist and alive
                let is_cow_alive_and_exist = self.state.is_cow_alive_and_exist(
                    cow_name.clone(), self.runtime.system_time(),
                ).await;
                if is_cow_alive_and_exist {
                    panic!("{:?} is not available", cow_name);
                }

                // make sure the cow is not exist in ownership
                let is_my_cow = self.state.cow_ownership.contains_key(&cow_name)
                    .await.unwrap_or(false);
                if is_my_cow {
                    panic!("you can't revive {:?}", cow_name);
                }

                // cow gender
                let cow_gender = self.random_cow_gender().await;

                // check if owner have enough balance to buy the cow
                let owner_balance = self.state.app_data.get().balance;
                let cow_price = self.state.get_cow_price(cow_breed);
                let balance_is_enough = owner_balance.gt(&cow_price);
                if !balance_is_enough {
                    panic!("{:?} don't have enough balance to buy {:?}", owner, cow_name);
                }

                // debit owner balance to pay for the cow
                self.state.debit(cow_price).await;

                // send BuyCow message to root chain
                let message = Message::BuyCow {
                    owner,
                    cow_buy_params: CowBuyParams::new(cow_id, cow_name, cow_breed, cow_gender, cow_price),
                };
                self.runtime
                    .prepare_message(message)
                    .with_authentication()
                    .with_tracking()
                    .send_to(self.runtime.application_parameters().root_chain_id);
            }
            CowOperation::FeedCow { owner, cow_name } => {
                log::info!("CowOperation::FeedCow");
                // check authentication
                self.check_authentication(owner);

                // Rule for FeedCow
                // we have 4 feeding time zone, that is every 6 hours.
                //
                // the time zones are:
                // 1st 6 hours -> FULL
                // 2nd 6 hours -> ON TIME
                // 3rd 6 hours -> LATE
                // 4th 6 hours -> FORGET
                //
                // if feeding distance are less than 6 hours, the cow won't eat, still full.
                // if feeding distance are more than 24 hours, the cow will die.

                // make sure the cow is existing both on local DB and in ownership
                // if not exist, probably the cow isn't yours to feed
                let is_cow_exist = self.state
                    .is_cow_exist_in_db_and_ownership(cow_name.clone()).await;
                if !is_cow_exist {
                    panic!("{:?} is not exist", cow_name);
                }

                // check cow name in DB and available to feed
                // you can't feed if cow already died
                let is_cow_alive_and_exist = self.state.is_cow_alive_and_exist(
                    cow_name.clone(), self.runtime.system_time(),
                ).await;
                if !is_cow_alive_and_exist {
                    panic!("you can't feed {:?}", cow_name);
                }

                // get CowData from DB
                let mut cow = self.state.cow_db.get(&cow_name).await
                    .unwrap_or_else(|_| {
                        panic!("unable to get Cow DB Result");
                    }).unwrap_or_else(|| {
                    panic!("unable to get Cow DB Option");
                });

                // find out feeding distance
                let current_time = self.runtime.system_time().micros();
                let last_fed_time = cow.last_fed_time.micros();
                let feed_distance = current_time - last_fed_time;

                if feed_distance <= WELL_FED {
                    panic!("{:?} still full", cow_name);
                }

                // calculate feeding stats
                let mut on_time = cow.feeding_stats.on_time;
                let mut late = cow.feeding_stats.late;
                let mut forgot = cow.feeding_stats.forgot;

                if feed_distance > WELL_FED && feed_distance <= ON_TIME_FEED {
                    on_time = on_time + 1;
                }
                if feed_distance > ON_TIME_FEED && feed_distance <= LATE_FEED {
                    late = late + 1;
                }
                if feed_distance > LATE_FEED {
                    forgot = forgot + 1;
                }

                // update cow data
                cow.last_fed_time = Timestamp::from(current_time);
                cow.feeding_stats = FeedingStats { on_time, late, forgot };

                // save data to db
                self.state.create_new_or_update_cow_data(cow_name, cow.clone()).await;

                // send FeedCow message to root chain
                let message = Message::FeedCow { owner, cow_data: cow };
                self.runtime
                    .prepare_message(message)
                    .with_authentication()
                    .send_to(self.runtime.application_parameters().root_chain_id);
            }
            CowOperation::SellCow { owner, cow_name, cow_born_time } => {
                log::info!("CowOperation::SellCow");
                // check authentication
                self.check_authentication(owner);

                // make sure the cow is existing both on local DB and in ownership
                // if not exist, probably the cow isn't yours to sell
                let is_cow_exist = self.state
                    .is_cow_exist_in_db_and_ownership(cow_name.clone()).await;
                if !is_cow_exist {
                    panic!("{:?} is not exist", cow_name);
                }

                // check cow name in DB and available to sell
                // you can't sell if cow already died
                let is_cow_alive_and_exist = self.state.is_cow_alive_and_exist(
                    cow_name.clone(), self.runtime.system_time(),
                ).await;
                if !is_cow_alive_and_exist {
                    panic!("you can't sell {:?}", cow_name);
                }

                // check if cow is underage
                let system_time = self.runtime.system_time();
                let is_cow_underage = self.state.is_cow_underage(cow_born_time, system_time).await;
                if is_cow_underage {
                    panic!("{:?} is too young to be sold", cow_name);
                }

                let message = Message::SellCow { owner, cow_name };
                self.runtime
                    .prepare_message(message)
                    .with_authentication()
                    .with_tracking()
                    .send_to(self.runtime.application_parameters().root_chain_id);
            }
            CowOperation::BuryDeadCows => {
                log::info!("CowOperation::BuryDeadCows");
                // get all of my cow
                let my_cows = self.state.get_my_cows().await;

                // filter cow who has died
                let current_time = self.runtime.system_time().micros();
                for cow in my_cows.into_iter() {
                    let last_fed_time = cow.last_fed_time.micros();
                    let last_fed_after_24_hours = last_fed_time + UNIX_MICROS_IN_24_HOURS;
                    if current_time.gt(&last_fed_after_24_hours) {
                        // cow last fed time is more than 24 hours, therefore it has died
                        // remove cow from DB
                        self.state.cow_db.remove(&cow.name).unwrap_or_else(|_| {
                            panic!("unable to remove Cow from DB");
                        });
                        self.state.cow_ownership.remove(&cow.name).unwrap_or_else(|_| {
                            panic!("unable to remove Cow from ownership");
                        });
                    }
                }
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        let is_bouncing = self
            .runtime
            .message_is_bouncing()
            .unwrap_or_else(|| {
                panic!("Message delivery status has to be available when executing a message");
            });

        let message_id = self
            .runtime
            .message_id()
            .unwrap_or_else(|| {
                panic!("Message ID has to be available when executing a message");
            });

        match _message {
            // ! executed by ROOT chain
            // ! --------------------------------------------------------------------------------
            Message::BuyCow { owner, cow_buy_params } => {
                if is_bouncing {
                    // ? BOUNCING parts executed by USER chain
                    // credit balance due to failure to BuyCow
                    log::info!("Message::BuyCow - Fail to Buy Cow: {:?}", cow_buy_params.name.clone());
                    self.state.credit(cow_buy_params.price).await;
                    self.state.buy_notification.push_back(
                        BuyNotif {
                            cow_name: cow_buy_params.name,
                            is_success: false,
                        });
                    return;
                }

                log::info!("Message::BuyCow");

                // check authentication
                self.check_authentication(owner);

                // check cow name in DB and available to buy
                // you can't buy if cow exist and alive
                let is_cow_alive_and_exist = self.state.is_cow_alive_and_exist(
                    cow_buy_params.name.clone(), self.runtime.system_time(),
                ).await;
                if is_cow_alive_and_exist {
                    let cow_data = self.state.cow_db.get(&cow_buy_params.name.clone()).await
                        .unwrap_or_else(|_| {
                            panic!("unable to get Cow DB Result");
                        }).unwrap_or_else(|| {
                        panic!("unable to get Cow DB Option");
                    });

                    let message = Message::BuyFailure { cow_data, cow_buy_params };
                    self.runtime
                        .prepare_message(message)
                        .send_to(message_id.chain_id);
                    return;
                }

                // new cow data.
                let new_cow_data = CowData {
                    id: cow_buy_params.id,
                    name: cow_buy_params.name.clone(),
                    breed: cow_buy_params.breed,
                    gender: cow_buy_params.gender,
                    born_time: self.runtime.system_time(),
                    last_fed_time: self.runtime.system_time(),
                    feeding_stats: FeedingStats::new(),
                    owner,
                };
                self.state.create_new_or_update_cow_data(cow_buy_params.name, new_cow_data.clone()).await;

                // credit balance to receive payment for the cow
                self.state.credit(cow_buy_params.price).await;

                // notify to channel subscriber that a BuyCow is success
                let message = Message::BuySuccess { cow_data: new_cow_data };
                self.runtime
                    .prepare_message(message)
                    .send_to(Destination::from(ChannelName::from(MICRO_COW_CHANNEL.to_vec())));
            }
            Message::FeedCow { owner, cow_data } => {
                log::info!("Message::FeedCow");
                // Message::FeedCow not being tracked
                // Even if it does, bouncing message should do nothing.
                if is_bouncing {
                    return;
                }

                // check authentication
                self.check_authentication(owner);

                // save data to db
                let cow_name = cow_data.name.clone();
                self.state.create_new_or_update_cow_data(cow_name, cow_data.clone()).await;

                // notify to channel subscriber that a FeedCow is success
                let message = Message::FeedSuccess { cow_data };
                self.runtime
                    .prepare_message(message)
                    .send_to(Destination::from(ChannelName::from(MICRO_COW_CHANNEL.to_vec())));
            }
            Message::SellCow { owner, cow_name } => {
                if is_bouncing {
                    // ? BOUNCING parts executed by USER chain
                    log::info!("Message::SellCow - Fail to Sell Cow: {:?}", cow_name);
                    self.state.sell_notification.push_back(SellNotif {
                        cow_name,
                        is_success: false,
                        failure_reason: String::from("Failure to sell, operation bounced"),
                    });
                    return;
                }

                log::info!("Message::SellCow");

                // check authentication
                self.check_authentication(owner);

                // get CowData from DB
                let cow = self.state.cow_db.get(&cow_name).await
                    .unwrap_or_else(|_| {
                        panic!("unable to get Cow DB Result");
                    }).unwrap_or_else(|| {
                    panic!("unable to get Cow DB Option");
                });

                // calculate cow selling price & check contract balance
                let cow_selling_price = self.state.cow_sell_value(cow.clone()).await;
                let contract_balance = self.state.app_data.get().balance;
                if contract_balance.lt(&cow_selling_price) {
                    let reason = String::from("Insufficient contract balance");
                    let message = Message::SellFailure { cow_name, reason };
                    self.runtime
                        .prepare_message(message)
                        .send_to(message_id.chain_id);
                    return;
                }

                // remove Cow from DB
                self.state.cow_db.remove(&cow_name).unwrap_or_else(|_| {
                    panic!("unable to remove Cow from DB");
                });

                // debit contract balance to pay for the cow
                self.state.debit(cow_selling_price.clone()).await;

                // notify to channel subscriber that a SellCow is success
                let message = Message::SellSuccess {
                    cow_name,
                    cow_owner: cow.owner,
                    payment: cow_selling_price,
                };
                self.runtime
                    .prepare_message(message)
                    .send_to(Destination::from(ChannelName::from(MICRO_COW_CHANNEL.to_vec())));
            }
            Message::Subscribe => {
                log::info!("Message::Subscribe");
                if is_bouncing {
                    // ? BOUNCING parts executed by USER chain
                    // nothing happens for now
                    return;
                }

                // register the Chain ID from message as subscriber for MICRO_COW_CHANNEL
                self.runtime.subscribe(
                    message_id.chain_id,
                    ChannelName::from(MICRO_COW_CHANNEL.to_vec()),
                );
            }
            // ! executed by USER chain
            // ! --------------------------------------------------------------------------------
            Message::BuySuccess { cow_data } => {
                log::info!("Message::BuySuccess");
                // Message::BuySuccess not being tracked
                // Even if it does, bouncing message should do nothing.
                if is_bouncing {
                    return;
                }

                // save new CowData to local state in all Micro Cow channel subscriber
                let cow_name = cow_data.name.clone();
                self.state.create_new_or_update_cow_data(cow_name.clone(), cow_data.clone()).await;


                // update Cow Ownership & Buy Notification only on Buyer's local state
                let owner = self.state.app_data.get().owner;
                if owner.eq(&cow_data.owner) {
                    self.state.update_cow_ownership(cow_name.clone()).await;
                    self.state.buy_notification.push_back(BuyNotif {
                        cow_name: cow_name.clone(),
                        is_success: true,
                    });
                } else {
                    // check Cow in subscriber's ownership, remove if it does exist.
                    let cow_in_ownership = self.state.cow_ownership.contains_key(&cow_name)
                        .await.unwrap_or(false);
                    if cow_in_ownership {
                        self.state.cow_ownership.remove(&cow_name).unwrap_or_else(|_| {
                            panic!("unable to remove Cow from ownership");
                        });
                    }
                }
            }
            Message::BuyFailure { cow_data, cow_buy_params } => {
                log::info!("Message::BuyFailure");
                // Message::BuyFailure not being tracked
                // Even if it does, bouncing message should do nothing.
                if is_bouncing {
                    return;
                }

                // credit balance due to failure to BuyCow
                self.state.credit(cow_buy_params.price).await;
                self.state.buy_notification.push_back(
                    BuyNotif {
                        cow_name: cow_buy_params.name,
                        is_success: false,
                    });

                // save CowData that we failed to buy to local state
                let cow_name = cow_data.name.clone();
                self.state.create_new_or_update_cow_data(cow_name, cow_data).await;
            }
            Message::FeedSuccess { cow_data } => {
                log::info!("Message::FeedSuccess");
                // Message::FeedSuccess not being tracked
                // Even if it does, bouncing message should do nothing.
                if is_bouncing {
                    return;
                }

                // check if CowData belong to us
                let cow_name = cow_data.name.clone();
                let is_my_cow = self.state
                    .is_cow_exist_in_db_and_ownership(cow_name.clone()).await;
                if is_my_cow {
                    // check if the cow is actually belong to us
                    let owner_id = self.state.app_data.get().owner;
                    if owner_id.ne(&cow_data.owner) {
                        self.state.cow_ownership.remove(&cow_data.name).unwrap_or_else(|_| {
                            panic!("unable to remove Cow from ownership");
                        });
                    }
                    // do nothing if all check passed
                    return;
                }

                // else, save data to db
                self.state.create_new_or_update_cow_data(cow_name, cow_data).await;
            }
            Message::SellFailure { cow_name, reason } => {
                log::info!("Message::SellFailure");
                // Message::SellFailure not being tracked
                // Even if it does, bouncing message should do nothing.
                if is_bouncing {
                    return;
                }

                self.state.sell_notification.push_back(SellNotif {
                    cow_name,
                    is_success: false,
                    failure_reason: reason,
                })
            }
            Message::SellSuccess { cow_name, cow_owner, payment } => {
                log::info!("Message::SellSuccess");
                // Message::SellSuccess not being tracked
                // Even if it does, bouncing message should do nothing.
                if is_bouncing {
                    return;
                }

                // update Cow Ownership & Sell Notification only on Seller's local state
                let owner = self.state.app_data.get().owner;
                if owner.eq(&cow_owner) {
                    // remove Cow from ownership
                    self.state.cow_ownership.remove(&cow_name).unwrap_or_else(|_| {
                        panic!("unable to remove Cow from ownership");
                    });

                    // credit balance to receive payment for the cow
                    self.state.credit(payment).await;

                    // push sell notification
                    self.state.sell_notification.push_back(SellNotif {
                        cow_name: cow_name.clone(),
                        is_success: true,
                        failure_reason: String::from(""),
                    });
                }

                // remove Cow from DB
                self.state.cow_db.remove(&cow_name).unwrap_or_else(|_| {
                    panic!("unable to remove Cow from DB");
                });
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// ------------------------------------------------------------------------------------------
impl MicroCowContract {
    fn check_authentication(&mut self, owner: Owner) {
        assert_eq!(
            self.runtime.authenticated_signer(),
            Some(owner),
            "Incorrect owner authentication"
        )
    }

    fn check_root_invocation(&mut self) {
        assert_ne!(
            self.runtime.chain_id(),
            self.runtime.application_parameters().root_chain_id,
            "Root are not allowed to play"
        )
    }

    async fn random_cow_gender(&mut self) -> CowGender {
        // produce seed array using system time
        let timestamp = self.runtime.system_time().to_string();
        let concatenated_timestamp = format!("{}{}{}", timestamp, timestamp, timestamp);
        let timestamp_str = truncate(concatenated_timestamp.as_str(), 32);
        let mut seed_array = [0u8; 32];
        seed_array = <[u8; 32]>::try_from(timestamp_str.as_bytes()).unwrap();

        // get random value using provided seed
        let mut buff = &mut [0];
        custom_getrandom(buff, seed_array).unwrap_or_else(|_| {
            panic!("failed random fill");
        });
        let val = buff.first().unwrap_or_else(|| {
            panic!("failed to get random value");
        });

        // check if random value is Even or Odd
        if val % 2 == 0 {
            return CowGender::Female;
        }
        CowGender::Male
    }

    fn subscribe_to_micro_cow_channel(&mut self) {
        let root_chain_id = self.runtime.application_parameters().root_chain_id;
        self.runtime
            .prepare_message(Message::Subscribe)
            .with_tracking()
            .send_to(root_chain_id);
    }
}