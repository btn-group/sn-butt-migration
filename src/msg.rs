use crate::state::{FillDetail, HumanizedOrder, SecretContract};
use cosmwasm_std::{Binary, HumanAddr, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub butt: SecretContract,
    pub mount_doom: SecretContract,
    pub execution_fee: Uint128,
    pub sscrt: SecretContract,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    CancelOrder {
        position: Uint128,
    },
    FillOrders {
        fill_details: Vec<FillDetail>,
    },
    Receive {
        sender: HumanAddr,
        from: HumanAddr,
        amount: Uint128,
        msg: Binary,
    },
    RegisterTokens {
        tokens: Vec<SecretContract>,
        viewing_key: String,
    },
    RescueTokens {
        denom: Option<String>,
        key: Option<String>,
        token_address: Option<HumanAddr>,
    },
    UpdateConfig {
        execution_fee: Uint128,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    Orders {
        orders: Vec<HumanizedOrder>,
        total: Option<Uint128>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Orders {
        address: HumanAddr,
        key: String,
        page: Uint128,
        page_size: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    SetExecutionFeeForOrder {},
    CreateOrder { to: HumanAddr },
}
