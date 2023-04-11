use crate::constants::PREFIX_REGISTERED_TOKENS;
use cosmwasm_std::{Api, CanonicalAddr, HumanAddr, StdResult, Storage, Uint128};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use schemars::JsonSchema;
use secret_toolkit::storage::{TypedStore, TypedStoreMut};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin: HumanAddr,
    pub butt: SecretContract,
    pub mount_doom: SecretContract,
    pub execution_fee: Uint128,
    pub sscrt: SecretContract,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct SecretContract {
    pub address: HumanAddr,
    pub contract_hash: String,
}

// === Registered tokens ===
#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, JsonSchema)]
pub struct RegisteredToken {
    pub address: HumanAddr,
    pub contract_hash: String,
    pub sum_balance: Uint128,
}

pub fn read_registered_token<S: Storage>(
    storage: &S,
    token_address: &CanonicalAddr,
) -> Option<RegisteredToken> {
    let registered_tokens_storage = ReadonlyPrefixedStorage::new(PREFIX_REGISTERED_TOKENS, storage);
    let registered_tokens_storage = TypedStore::attach(&registered_tokens_storage);
    registered_tokens_storage
        .may_load(token_address.as_slice())
        .unwrap()
}

pub fn write_registered_token<S: Storage>(
    storage: &mut S,
    token_address: &CanonicalAddr,
    registered_token: &RegisteredToken,
) -> StdResult<()> {
    let mut registered_tokens_storage = PrefixedStorage::new(PREFIX_REGISTERED_TOKENS, storage);
    let mut registered_tokens_storage = TypedStoreMut::attach(&mut registered_tokens_storage);
    registered_tokens_storage.store(token_address.as_slice(), registered_token)
}

// === Orders ===
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct HumanizedOrder {
    pub position: Uint128,
    pub execution_fee: Option<Uint128>,
    pub creator: HumanAddr,
    pub amount: Uint128,
    pub to: HumanAddr,
    pub status: u8,
    pub created_at_block_time: u64,
    pub created_at_block_height: u64,
}

// activity (0 => open, 1 => filled, 2 => cancelled)
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
pub struct Order {
    pub position: Uint128,
    pub execution_fee: Option<Uint128>,
    pub other_storage_position: Uint128,
    pub creator: CanonicalAddr,
    pub amount: Uint128,
    pub to: HumanAddr,
    pub status: u8,
    pub created_at_block_time: u64,
    pub created_at_block_height: u64,
}
impl Order {
    pub fn into_humanized<A: Api>(self, api: &A) -> StdResult<HumanizedOrder> {
        Ok(HumanizedOrder {
            position: self.position,
            execution_fee: self.execution_fee,
            creator: api.human_address(&self.creator)?,
            amount: self.amount,
            to: self.to,
            status: self.status,
            created_at_block_time: self.created_at_block_time,
            created_at_block_height: self.created_at_block_height,
        })
    }
}
