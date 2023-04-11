use crate::constants::{
    BLOCK_SIZE, CONFIG_KEY, MOCK_AMOUNT, MOCK_BUTT_ADDRESS, MOCK_TOKEN_ADDRESS, PREFIX_ORDERS,
    PREFIX_ORDERS_COUNT,
};
use crate::msg::{HandleMsg, InitMsg, QueryAnswer, QueryMsg, ReceiveMsg};
use crate::state::{
    read_registered_token, write_registered_token, Config, HumanizedOrder, Order, RegisteredToken,
    SecretContract,
};
use crate::validations::{authorize, validate_human_addr, validate_uint128};
use cosmwasm_std::{
    from_binary, to_binary, Api, BalanceResponse, BankMsg, BankQuery, Binary, CanonicalAddr, Coin,
    CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse, Querier, QueryRequest,
    ReadonlyStorage, StdError, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

use secret_toolkit::snip20;
use secret_toolkit::storage::{TypedStore, TypedStoreMut};

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let config: Config = Config {
        admin: env.message.sender,
        butt: msg.butt,
        mount_doom: msg.mount_doom,
        execution_fee: msg.execution_fee,
        sscrt: msg.sscrt,
    };
    config_store.store(CONFIG_KEY, &config)?;

    Ok(InitResponse {
        messages: vec![],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::CancelOrder { position } => cancel_order(deps, &env, position.u128()),
        HandleMsg::Receive {
            from, amount, msg, ..
        } => receive(deps, env, from, amount, msg),
        HandleMsg::RegisterTokens {
            tokens,
            viewing_key,
        } => register_tokens(deps, &env, tokens, viewing_key),
        HandleMsg::RescueTokens {
            denom,
            key,
            token_address,
        } => rescue_tokens(deps, &env, denom, key, token_address),
        HandleMsg::UpdateConfig { execution_fee } => update_config(deps, &env, execution_fee),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => {
            let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY)?;
            Ok(to_binary(&config)?)
        }
        QueryMsg::Orders {
            address,
            key,
            page,
            page_size,
        } => orders(deps, address, key, page.u128(), page_size.u128()),
        QueryMsg::OrdersByPositions {
            address,
            key,
            positions,
        } => orders_by_positions(deps, address, key, positions),
    }
}

fn receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    from: HumanAddr,
    amount: Uint128,
    msg: Binary,
) -> StdResult<HandleResponse> {
    let msg: ReceiveMsg = from_binary(&msg)?;
    let response = match msg {
        ReceiveMsg::SetExecutionFeeForOrder {} => {
            set_execution_fee_for_order(deps, &env, from, amount)
        }
        ReceiveMsg::CreateOrder { to } => create_order(deps, &env, from, amount, to),
    };
    pad_response(response)
}

fn set_execution_fee_for_order<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    from: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    validate_human_addr(
        &config.sscrt.address,
        &env.message.sender,
        "Execution fee token must be SSCRT.",
    )?;
    validate_uint128(
        config.execution_fee,
        amount,
        "Amount sent in must equal execution fee.",
    )?;

    let contract_canonical_address: CanonicalAddr =
        deps.api.canonical_address(&env.contract.address)?;
    let user_canonical_address: CanonicalAddr = deps.api.canonical_address(&from)?;
    let next_order_position: u128 =
        storage_count(&deps.storage, &user_canonical_address, PREFIX_ORDERS_COUNT)?;
    let order_position: u128 = if next_order_position == 0 {
        return Err(StdError::generic_err("Order does not exist."));
    } else {
        next_order_position - 1
    };
    let mut creator_order =
        order_at_position(&deps.storage, &user_canonical_address, order_position)?;
    validate_uint128(
        Uint128::from(creator_order.created_at_block_height),
        Uint128::from(env.block.height),
        "Execution fee must be set at the same block as when order is created.",
    )?;

    if creator_order.execution_fee.is_some() {
        return Err(StdError::generic_err(
            "Execution fee already set for order.",
        ));
    }
    if creator_order.status != 0 {
        return Err(StdError::generic_err("Order is not open."));
    }

    creator_order.execution_fee = Some(amount);
    update_creator_order_and_associated_contract_order(
        &mut deps.storage,
        &user_canonical_address,
        creator_order.clone(),
        &contract_canonical_address,
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&creator_order.into_humanized(&deps.api)?)?),
    })
}

fn set_count<S: Storage>(
    store: &mut S,
    for_address: &CanonicalAddr,
    storage_prefix: &[u8],
    count: u128,
) -> StdResult<()> {
    let mut prefixed_store = PrefixedStorage::new(storage_prefix, store);
    let mut count_store = TypedStoreMut::<u128, _>::attach(&mut prefixed_store);
    count_store.store(for_address.as_slice(), &count)
}

fn append_order<S: Storage>(
    store: &mut S,
    order: &Order,
    for_address: &CanonicalAddr,
) -> StdResult<()> {
    let mut prefixed_store =
        PrefixedStorage::multilevel(&[PREFIX_ORDERS, for_address.as_slice()], store);
    let mut order_store = TypedStoreMut::<Order, _>::attach(&mut prefixed_store);
    order_store.store(&order.position.u128().to_le_bytes(), order)?;
    set_count(
        store,
        for_address,
        PREFIX_ORDERS_COUNT,
        order.position.u128().checked_add(1).ok_or_else(|| {
            StdError::generic_err(
                "Reached implementation limit for the number of orders per address.",
            )
        })?,
    )
}

fn cancel_order<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    position: u128,
) -> StdResult<HandleResponse> {
    let contract_canonical_address: CanonicalAddr =
        deps.api.canonical_address(&env.contract.address)?;
    let mut creator_order = order_at_position(
        &deps.storage,
        &deps.api.canonical_address(&env.message.sender)?,
        position,
    )?;
    if creator_order.status != 0 {
        return Err(StdError::generic_err(
            "Order is already filled or cancelled.",
        ));
    }

    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    // Send refund to the creator
    let mut messages: Vec<CosmosMsg> = vec![];
    messages.push(snip20::transfer_msg(
        env.message.sender.clone(),
        creator_order.amount,
        None,
        BLOCK_SIZE,
        config.butt.contract_hash,
        config.butt.address,
    )?);

    // Update Txs
    creator_order.status = 2;
    update_creator_order_and_associated_contract_order(
        &mut deps.storage,
        &creator_order.creator,
        creator_order.clone(),
        &contract_canonical_address,
    )?;

    // If order has an execution fee send it back to the user
    if let Some(execution_fee_unwrapped) = creator_order.execution_fee {
        messages.push(snip20::transfer_msg(
            env.message.sender.clone(),
            execution_fee_unwrapped,
            None,
            BLOCK_SIZE,
            config.sscrt.contract_hash,
            config.sscrt.address,
        )?);
    }

    pad_response(Ok(HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&creator_order.into_humanized(&deps.api)?)?),
    }))
}

// activity (0 => open, 1 => filled, 2 => cancelled)
fn create_order<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    from: HumanAddr,
    amount: Uint128,
    to: HumanAddr,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    authorize(vec![config.butt.address], &env.message.sender)?;

    // Store order
    let contract_address: CanonicalAddr = deps.api.canonical_address(&env.contract.address)?;
    let creator_address: CanonicalAddr = deps.api.canonical_address(&from)?;
    let contract_order_position =
        storage_count(&deps.storage, &contract_address, PREFIX_ORDERS_COUNT)?;
    let creator_order_position =
        storage_count(&deps.storage, &creator_address, PREFIX_ORDERS_COUNT)?;
    // Store contract order first
    let mut order = Order {
        position: Uint128(contract_order_position),
        execution_fee: None,
        other_storage_position: Uint128(creator_order_position),
        creator: creator_address.clone(),
        amount,
        to,
        status: 0,
        created_at_block_time: env.block.time,
        created_at_block_height: env.block.height,
    };
    append_order(&mut deps.storage, &order, &contract_address)?;
    // Store creator order next
    order.position = Uint128(creator_order_position);
    order.other_storage_position = Uint128(contract_order_position);
    append_order(&mut deps.storage, &order, &creator_address)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&order.into_humanized(&deps.api)?)?),
    })
}

fn get_orders<A: Api, S: ReadonlyStorage>(
    api: &A,
    storage: &S,
    for_address: &CanonicalAddr,
    page: u128,
    page_size: u128,
) -> StdResult<(Vec<HumanizedOrder>, u128)> {
    let total: u128 = storage_count(storage, for_address, PREFIX_ORDERS_COUNT)?;
    let offset: u128 = page * page_size;
    let end = total - offset;
    let start = end.saturating_sub(page_size);
    let store =
        ReadonlyPrefixedStorage::multilevel(&[PREFIX_ORDERS, for_address.as_slice()], storage);
    let mut orders: Vec<HumanizedOrder> = Vec::new();
    let store = TypedStore::<Order, _>::attach(&store);
    for position in (start..end).rev() {
        orders.push(store.load(&position.to_le_bytes())?.into_humanized(api)?);
    }

    Ok((orders, total))
}

fn order_at_position<S: Storage>(
    store: &S,
    address: &CanonicalAddr,
    position: u128,
) -> StdResult<Order> {
    let store = ReadonlyPrefixedStorage::multilevel(&[PREFIX_ORDERS, address.as_slice()], store);
    // Try to access the storage of orders for the account.
    // If it doesn't exist yet, return an empty list of transfers.
    let store = TypedStore::<Order, _>::attach(&store);

    store.load(&position.to_le_bytes())
}

fn orders<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    key: String,
    page: u128,
    page_size: u128,
) -> StdResult<Binary> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    // This is here so that the user can use their viewing key for butt for this
    query_balance_of_token(deps, address.clone(), config.butt, key)?;

    let (orders, total) = get_orders(
        &deps.api,
        &deps.storage,
        &deps.api.canonical_address(&address)?,
        page,
        page_size,
    )?;

    let result = QueryAnswer::Orders {
        orders,
        total: Some(Uint128(total)),
    };
    to_binary(&result)
}

fn orders_by_positions<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    key: String,
    positions: Vec<Uint128>,
) -> StdResult<Binary> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    query_balance_of_token(deps, address.clone(), config.butt, key)?;

    let address = deps.api.canonical_address(&address)?;
    let mut orders: Vec<HumanizedOrder> = vec![];
    for position in positions.iter() {
        let order = order_at_position(&deps.storage, &address, position.u128())?;
        orders.push(order.into_humanized(&deps.api)?)
    }

    let result = QueryAnswer::Orders {
        orders,
        total: None,
    };
    to_binary(&result)
}

fn pad_response(response: StdResult<HandleResponse>) -> StdResult<HandleResponse> {
    response.map(|mut response| {
        response.data = response.data.map(|mut data| {
            space_pad(BLOCK_SIZE, &mut data.0);
            data
        });
        response
    })
}

fn query_balance_of_token<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
    token: SecretContract,
    viewing_key: String,
) -> StdResult<Uint128> {
    if token.address == HumanAddr::from(MOCK_TOKEN_ADDRESS)
        || token.address == HumanAddr::from(MOCK_BUTT_ADDRESS)
    {
        Ok(Uint128(MOCK_AMOUNT))
    } else {
        let balance = snip20::balance_query(
            &deps.querier,
            address,
            viewing_key,
            BLOCK_SIZE,
            token.contract_hash,
            token.address,
        )?;
        Ok(balance.amount)
    }
}

fn register_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    tokens: Vec<SecretContract>,
    viewing_key: String,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin], &env.message.sender)?;
    let mut messages = vec![];
    for token in tokens {
        let token_address_canonical = deps.api.canonical_address(&token.address)?;
        let token_details: Option<RegisteredToken> =
            read_registered_token(&deps.storage, &token_address_canonical);
        if token_details.is_none() {
            let token_details: RegisteredToken = RegisteredToken {
                address: token.address.clone(),
                contract_hash: token.contract_hash.clone(),
            };
            write_registered_token(&mut deps.storage, &token_address_canonical, &token_details)?;
            messages.push(snip20::register_receive_msg(
                env.contract_code_hash.clone(),
                None,
                BLOCK_SIZE,
                token.contract_hash.clone(),
                token.address.clone(),
            )?);
        }
        messages.push(snip20::set_viewing_key_msg(
            viewing_key.clone(),
            None,
            BLOCK_SIZE,
            token.contract_hash,
            token.address,
        )?);
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

fn rescue_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    denom: Option<String>,
    key: Option<String>,
    token_address: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin.clone()], &env.message.sender)?;

    let mut messages: Vec<CosmosMsg> = vec![];
    if let Some(denom_unwrapped) = denom {
        let balance_response: BalanceResponse =
            deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
                address: env.contract.address.clone(),
                denom: denom_unwrapped,
            }))?;

        let withdrawal_coins: Vec<Coin> = vec![balance_response.amount];
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: config.admin.clone(),
            amount: withdrawal_coins,
        }));
    }

    if let Some(token_address_unwrapped) = token_address {
        if let Some(key_unwrapped) = key {
            let registered_token: RegisteredToken = read_registered_token(
                &deps.storage,
                &deps.api.canonical_address(&token_address_unwrapped)?,
            )
            .unwrap();
            let balance: Uint128 = query_balance_of_token(
                deps,
                env.contract.address.clone(),
                SecretContract {
                    address: token_address_unwrapped,
                    contract_hash: registered_token.contract_hash.clone(),
                },
                key_unwrapped,
            )?;
            messages.push(snip20::transfer_msg(
                config.admin,
                balance,
                None,
                BLOCK_SIZE,
                registered_token.contract_hash,
                registered_token.address,
            )?)
        }
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

// Take a Vec<u8> and pad it up to a multiple of `block_size`, using spaces at the end.
fn space_pad(block_size: usize, message: &mut Vec<u8>) -> &mut Vec<u8> {
    let len = message.len();
    let surplus = len % block_size;
    if surplus == 0 {
        return message;
    }

    let missing = block_size - surplus;
    message.reserve(missing);
    message.extend(std::iter::repeat(b' ').take(missing));
    message
}

fn storage_count<S: ReadonlyStorage>(
    store: &S,
    for_address: &CanonicalAddr,
    storage_prefix: &[u8],
) -> StdResult<u128> {
    let store = ReadonlyPrefixedStorage::new(storage_prefix, store);
    let store = TypedStore::<u128, _>::attach(&store);
    let position: Option<u128> = store.may_load(for_address.as_slice())?;

    Ok(position.unwrap_or(0))
}

fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    execution_fee: Uint128,
) -> StdResult<HandleResponse> {
    let mut config_store = TypedStoreMut::attach(&mut deps.storage);
    let mut config: Config = config_store.load(CONFIG_KEY).unwrap();
    authorize(vec![config.admin.clone()], &env.message.sender)?;

    config.execution_fee = execution_fee;
    config_store.store(CONFIG_KEY, &config)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: None,
    })
}

fn update_creator_order_and_associated_contract_order<S: Storage>(
    store: &mut S,
    user_address: &CanonicalAddr,
    creator_order: Order,
    contract_address: &CanonicalAddr,
) -> StdResult<()> {
    let mut user_store =
        PrefixedStorage::multilevel(&[PREFIX_ORDERS, user_address.as_slice()], store);
    // Try to access the storage of orders for the account.
    // If it doesn't exist yet, return an empty list of transfers.
    let mut user_store = TypedStoreMut::<Order, _, _>::attach(&mut user_store);
    user_store.store(&creator_order.position.u128().to_le_bytes(), &creator_order)?;
    let mut contract_store =
        PrefixedStorage::multilevel(&[PREFIX_ORDERS, contract_address.as_slice()], store);
    let mut contract_store = TypedStoreMut::<Order, _, _>::attach(&mut contract_store);
    let contract_order_position: Uint128 = creator_order.other_storage_position;
    let creator_order_position: Uint128 = creator_order.position;
    let mut contract_order = creator_order;
    contract_order.position = contract_order_position;
    contract_order.other_storage_position = creator_order_position;
    contract_store.store(
        &contract_order.position.u128().to_le_bytes(),
        &contract_order,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SecretContract;
    use cosmwasm_std::from_binary;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::StdError::NotFound;

    pub const MOCK_ADMIN: &str = "admin";
    pub const MOCK_MOUNT_DOOM_ADDRESS: &str = "mock-mount-doom-contract-hash-address";
    pub const MOCK_VIEWING_KEY: &str = "DELIGHTFUL";
    pub const MOCK_SSCRT_ADDRESS: &str = "mock-sscrt-address";

    // === HELPERS ===
    fn create_order_helper<S: Storage, A: Api, Q: Querier>(deps: &mut Extern<S, A, Q>) {
        let receive_msg = ReceiveMsg::CreateOrder {
            to: mock_token().address,
        };
        let handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount: Uint128(MOCK_AMOUNT),
            msg: to_binary(&receive_msg).unwrap(),
        };
        handle(deps, mock_env(mock_butt().address, &[]), handle_msg.clone()).unwrap();
    }

    fn init_helper(
        register_tokens: bool,
    ) -> (
        StdResult<InitResponse>,
        Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        let env = mock_env(MOCK_ADMIN, &[]);
        let mut deps = mock_dependencies(20, &[]);
        let msg = InitMsg {
            butt: mock_butt(),
            execution_fee: mock_execution_fee(),
            mount_doom: mock_mount_doom(),
            sscrt: mock_sscrt(),
        };
        let init_result = init(&mut deps, env.clone(), msg);
        if register_tokens {
            let handle_msg = HandleMsg::RegisterTokens {
                tokens: vec![mock_butt(), mock_token()],
                viewing_key: MOCK_VIEWING_KEY.to_string(),
            };
            handle(&mut deps, env, handle_msg.clone()).unwrap();
        }
        (init_result, deps)
    }

    fn mock_butt() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_BUTT_ADDRESS),
            contract_hash: "mock-butt-contract-hash".to_string(),
        }
    }

    fn mock_contract() -> SecretContract {
        let env = mock_env(mock_user_address(), &[]);
        SecretContract {
            address: env.contract.address,
            contract_hash: env.contract_code_hash,
        }
    }

    fn mock_execution_fee() -> Uint128 {
        Uint128(5_555)
    }

    fn mock_mount_doom() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_MOUNT_DOOM_ADDRESS),
            contract_hash: "mock-mount-doom-contract-hash".to_string(),
        }
    }

    fn mock_sscrt() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_SSCRT_ADDRESS),
            contract_hash: "mock-sscrt-contract-hash".to_string(),
        }
    }

    fn mock_token() -> SecretContract {
        SecretContract {
            address: HumanAddr::from(MOCK_TOKEN_ADDRESS),
            contract_hash: "mock-token-contract-hash".to_string(),
        }
    }

    fn mock_user_address() -> HumanAddr {
        HumanAddr::from("gary")
    }

    // === UNIT TESTS ===
    #[test]
    fn test_set_execution_fee_for_order() {
        let (_init_result, mut deps) = init_helper(true);
        let mut env = mock_env(mock_butt().address, &[]);

        // when token sent in is not sscrt
        let receive_msg = ReceiveMsg::SetExecutionFeeForOrder {};
        let handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount: Uint128(1),
            msg: to_binary(&receive_msg).unwrap(),
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Execution fee token must be SSCRT.")
        );

        // when token sent in is sscrt
        env = mock_env(mock_sscrt().address, &[]);
        // = when amount sent in is not equal to execution fee
        let handle_result = handle(&mut deps, env.clone(), handle_msg);
        // * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Amount sent in must equal execution fee.")
        );
        // = when amount sent in is equal to execution fee
        let handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount: mock_execution_fee(),
            msg: to_binary(&receive_msg).unwrap(),
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg);
        // == when user does not have any orders
        // === * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Order does not exist.")
        );
        // == when user has at least one order
        create_order_helper(&mut deps);
        // === when current block is the same as the block when the order is created
        // ==== when order has fee set already
        let mut creator_order = order_at_position(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            0,
        )
        .unwrap();
        creator_order.execution_fee = Some(Uint128(1));
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // ==== * it raises an error
        let handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount: mock_execution_fee(),
            msg: to_binary(&receive_msg).unwrap(),
        };
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Execution fee already set for order.")
        );
        // ==== when order does not have execution fee set already
        // ===== when order is cancelled
        creator_order.execution_fee = None;
        creator_order.status = 2;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // ===== * it raises an error
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Order is not open.")
        );
        // ===== when order is filled
        creator_order.status = 1;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // ===== * it raises an error
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Order is not open.")
        );
        // ===== when order is open
        creator_order.status = 0;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // ===== * it sets the execution fee for the user order
        let handle_unwrapped = handle(&mut deps, env.clone(), handle_msg.clone()).unwrap();
        creator_order = order_at_position(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(creator_order.execution_fee, Some(mock_execution_fee()));
        // ===== * it sets the execution fee for the contract order
        let contract_order = order_at_position(
            &mut deps.storage,
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
            creator_order.other_storage_position.u128(),
        )
        .unwrap();
        assert_eq!(contract_order.execution_fee, Some(mock_execution_fee()));
        // ===== * it sends the humanized creator order back as data
        assert_eq!(
            handle_unwrapped.data,
            pad_response(Ok(HandleResponse {
                messages: vec![],
                log: vec![],
                data: Some(
                    to_binary(&creator_order.clone().into_humanized(&deps.api).unwrap()).unwrap()
                ),
            }))
            .unwrap()
            .data
        );

        // === when current block is different from the block when the order is created
        let mut creator_order = order_at_position(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            0,
        )
        .unwrap();
        creator_order.created_at_block_height = 1;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // === * it raises an error
        let handle_result = handle(&mut deps, env.clone(), handle_msg);
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err(
                "Execution fee must be set at the same block as when order is created."
            )
        );
    }

    #[test]
    fn test_cancel_order() {
        let (_init_result, mut deps) = init_helper(true);
        let env = mock_env(mock_user_address(), &[]);

        // = when order at position does not exist
        let mut handle_msg = HandleMsg::CancelOrder {
            position: Uint128(0),
        };
        let mut handle_result = handle(&mut deps, env.clone(), handle_msg.clone());

        // = * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            NotFound {
                kind: "cw_secret_network_butt_migration::state::Order".to_string(),
                backtrace: None
            }
        );

        // = when order at position exists
        create_order_helper(&mut deps);
        handle_msg = HandleMsg::CancelOrder {
            position: Uint128(0),
        };
        // === when order is cancelled
        let mut creator_order = order_at_position(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            0,
        )
        .unwrap();
        creator_order.status = 2;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &creator_order.creator,
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // === * it raises an error
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Order is already filled or cancelled.")
        );
        // === when order is filled
        creator_order.status = 2;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &creator_order.creator,
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // === * it raises an error
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::generic_err("Order is already filled or cancelled.")
        );
        // === when order can be cancelled
        creator_order.status = 0;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &creator_order.creator,
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        // === * it sends the unfilled from token amount back to the creator
        let handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![snip20::transfer_msg(
                deps.api.human_address(&creator_order.creator).unwrap(),
                creator_order.amount,
                None,
                BLOCK_SIZE,
                mock_butt().contract_hash.clone(),
                mock_butt().address.clone(),
            )
            .unwrap()]
        );
        // === * it sends the creator order as humanized back as data
        let creator_order = order_at_position(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(
            handle_result_unwrapped.data,
            pad_response(Ok(HandleResponse {
                messages: vec![],
                log: vec![],
                data: Some(
                    to_binary(&creator_order.clone().into_humanized(&deps.api).unwrap()).unwrap()
                ),
            }))
            .unwrap()
            .data
        );

        // === * it sets cancelled to true
        let mut creator_order = order_at_position(
            &mut deps.storage,
            &deps.api.canonical_address(&mock_user_address()).unwrap(),
            0,
        )
        .unwrap();
        let contract_order = order_at_position(
            &mut deps.storage,
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
            creator_order.other_storage_position.u128(),
        )
        .unwrap();
        assert_eq!(creator_order.status, 2);
        assert_eq!(contract_order.status, 2);

        // ==== when order has an execution fee
        creator_order.execution_fee = Some(Uint128(1));
        creator_order.status = 0;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &creator_order.creator,
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        creator_order.execution_fee = Some(Uint128(1));
        creator_order.status = 0;
        update_creator_order_and_associated_contract_order(
            &mut deps.storage,
            &creator_order.creator,
            creator_order.clone(),
            &deps
                .api
                .canonical_address(&mock_contract().address)
                .unwrap(),
        )
        .unwrap();
        // ===== * it sends the execution fee back to the creator
        handle_result = handle(&mut deps, env.clone(), handle_msg);
        assert_eq!(
            handle_result.unwrap().messages,
            vec![
                snip20::transfer_msg(
                    deps.api.human_address(&creator_order.creator).unwrap(),
                    creator_order.amount,
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                snip20::transfer_msg(
                    deps.api.human_address(&creator_order.creator).unwrap(),
                    creator_order.execution_fee.unwrap(),
                    None,
                    BLOCK_SIZE,
                    mock_sscrt().contract_hash,
                    mock_sscrt().address,
                )
                .unwrap()
            ]
        );
    }

    #[test]
    fn test_config() {
        let (_init_result, deps) = init_helper(false);

        let res = query(&deps, QueryMsg::Config {}).unwrap();
        let value: Config = from_binary(&res).unwrap();
        assert_eq!(
            Config {
                admin: HumanAddr::from(MOCK_ADMIN),
                butt: mock_butt(),
                execution_fee: mock_execution_fee(),
                mount_doom: mock_mount_doom(),
                sscrt: mock_sscrt(),
            },
            value
        );
    }

    #[test]
    fn test_create_order() {
        let (_init_result, mut deps) = init_helper(true);
        let receive_msg = ReceiveMsg::CreateOrder {
            to: mock_token().address,
        };
        let handle_msg = HandleMsg::Receive {
            sender: mock_user_address(),
            from: mock_user_address(),
            amount: Uint128(MOCK_AMOUNT),
            msg: to_binary(&receive_msg).unwrap(),
        };

        // = when token sent in isn't BUTT
        let handle_result = handle(
            &mut deps,
            mock_env(mock_contract().address, &[]),
            handle_msg.clone(),
        );
        // = * it raises an error
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // = when token sent in is BUTT
        let handle_unwrapped = handle(
            &mut deps,
            mock_env(mock_butt().address, &[]),
            handle_msg.clone(),
        )
        .unwrap();
        // === * it sends the humanized creator order back as data
        let order: Order = Order {
            position: Uint128(0),
            execution_fee: None,
            other_storage_position: Uint128(0),
            creator: deps.api.canonical_address(&mock_user_address()).unwrap(),
            amount: Uint128(MOCK_AMOUNT),
            to: mock_token().address,
            status: 0,
            created_at_block_time: mock_env(MOCK_ADMIN, &[]).block.time,
            created_at_block_height: mock_env(MOCK_ADMIN, &[]).block.height,
        };
        assert_eq!(
            handle_unwrapped.data,
            pad_response(Ok(HandleResponse {
                messages: vec![],
                log: vec![],
                data: Some(to_binary(&order.clone().into_humanized(&deps.api).unwrap()).unwrap()),
            }))
            .unwrap()
            .data
        );

        // === * it stores the order for the creator
        // === * it stores the order for the smart_contract
        assert_eq!(
            order_at_position(
                &mut deps.storage,
                &deps.api.canonical_address(&mock_user_address()).unwrap(),
                0
            )
            .unwrap(),
            order
        );
        assert_eq!(
            order_at_position(
                &mut deps.storage,
                &deps
                    .api
                    .canonical_address(&mock_contract().address)
                    .unwrap(),
                0
            )
            .unwrap(),
            order
        )
    }

    #[test]
    fn test_orders_by_positions() {
        let (_init_result, mut deps) = init_helper(true);

        // when user's address and butt viewing key combo is correct
        // = when user does not have any orders yet
        // = * it raises an error
        let mut res = query(
            &deps,
            QueryMsg::OrdersByPositions {
                address: mock_user_address(),
                key: MOCK_VIEWING_KEY.to_string(),
                positions: vec![Uint128(0)],
            },
        );
        assert_eq!(
            res.unwrap_err(),
            NotFound {
                kind: "cw_secret_network_butt_migration::state::Order".to_string(),
                backtrace: None
            }
        );

        // = when user has orders
        create_order_helper(&mut deps);
        create_order_helper(&mut deps);
        create_order_helper(&mut deps);
        create_order_helper(&mut deps);
        create_order_helper(&mut deps);
        // == when position requested is unavailable
        res = query(
            &deps,
            QueryMsg::OrdersByPositions {
                address: mock_user_address(),
                key: MOCK_VIEWING_KEY.to_string(),
                positions: vec![Uint128(1), Uint128(2), Uint128(3), Uint128(5)],
            },
        );
        assert_eq!(
            res.unwrap_err(),
            NotFound {
                kind: "cw_secret_network_butt_migration::state::Order".to_string(),
                backtrace: None
            }
        );
        // == when position requested is available
        res = query(
            &deps,
            QueryMsg::OrdersByPositions {
                address: mock_user_address(),
                key: MOCK_VIEWING_KEY.to_string(),
                positions: vec![Uint128(1), Uint128(3), Uint128(4)],
            },
        );
        // == * it returns the humanized orders at those positions
        let query_answer: QueryAnswer = from_binary(&res.unwrap()).unwrap();
        match query_answer {
            QueryAnswer::Orders { orders, total } => {
                assert_eq!(total, None);
                assert_eq!(orders[0].creator, mock_user_address());
                assert_eq!(orders[0].position, Uint128(1));
                assert_eq!(orders[1].position, Uint128(3));
                assert_eq!(orders[2].position, Uint128(4));
            }
        };
    }

    #[test]
    fn test_register_tokens() {
        let (_init_result, mut deps) = init_helper(false);

        // When tokens are in the parameter
        let handle_msg = HandleMsg::RegisterTokens {
            tokens: vec![mock_butt(), mock_token()],
            viewing_key: MOCK_VIEWING_KEY.to_string(),
        };
        // = when called by a non-admin
        // = * it raises an Unauthorized error
        let handle_result = handle(
            &mut deps,
            mock_env(mock_user_address(), &[]),
            handle_msg.clone(),
        );
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // = when called by the admin
        let handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        // == when tokens are not registered
        // == * it stores the registered tokens
        assert_eq!(
            read_registered_token(
                &deps.storage,
                &deps.api.canonical_address(&mock_butt().address).unwrap()
            )
            .is_some(),
            true
        );
        assert_eq!(
            read_registered_token(
                &deps.storage,
                &deps.api.canonical_address(&mock_token().address).unwrap()
            )
            .is_some(),
            true
        );

        // == * it registers the contract with the tokens
        // == * it sets the viewing key for the contract with the tokens
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::register_receive_msg(
                    mock_contract().contract_hash.clone(),
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                snip20::register_receive_msg(
                    mock_contract().contract_hash,
                    None,
                    BLOCK_SIZE,
                    mock_token().contract_hash,
                    mock_token().address,
                )
                .unwrap(),
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    BLOCK_SIZE,
                    mock_token().contract_hash,
                    mock_token().address,
                )
                .unwrap()
            ]
        );

        // === context when tokens are registered
        let handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg);
        let handle_result_unwrapped = handle_result.unwrap();
        // === * it sets the viewing key for the contract with the tokens
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    BLOCK_SIZE,
                    mock_butt().contract_hash,
                    mock_butt().address,
                )
                .unwrap(),
                snip20::set_viewing_key_msg(
                    MOCK_VIEWING_KEY.to_string(),
                    None,
                    BLOCK_SIZE,
                    mock_token().contract_hash,
                    mock_token().address,
                )
                .unwrap()
            ]
        );
    }

    #[test]
    fn test_rescue_tokens() {
        let (_init_result, mut deps) = init_helper(true);
        let handle_msg = HandleMsg::RescueTokens {
            denom: Some("uscrt".to_string()),
            key: Some(MOCK_VIEWING_KEY.to_string()),
            token_address: Some(mock_butt().address),
        };
        // = when called by a non-admin
        // = * it raises an Unauthorized error
        let handle_result = handle(
            &mut deps,
            mock_env(mock_user_address(), &[]),
            handle_msg.clone(),
        );
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // = when called by the admin
        // == when only denom is specified
        let handle_msg = HandleMsg::RescueTokens {
            denom: Some("uscrt".to_string()),
            key: None,
            token_address: None,
        };
        // === when the contract does not have the coin in it
        // === * it sends a transfer with the balance of the coin for the contract
        let handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![CosmosMsg::Bank(BankMsg::Send {
                from_address: mock_contract().address,
                to_address: HumanAddr(MOCK_ADMIN.to_string()),
                amount: vec![Coin {
                    denom: "uscrt".to_string(),
                    amount: Uint128(0)
                }],
            })]
        );

        // == when only token address and key are specified
        let handle_msg = HandleMsg::RescueTokens {
            denom: None,
            key: Some(MOCK_VIEWING_KEY.to_string()),
            token_address: Some(mock_butt().address),
        };
        // == * it sends the excess amount of token
        let handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
        let handle_result_unwrapped = handle_result.unwrap();
        assert_eq!(
            handle_result_unwrapped.messages,
            vec![snip20::transfer_msg(
                HumanAddr::from(MOCK_ADMIN),
                Uint128(MOCK_AMOUNT),
                None,
                BLOCK_SIZE,
                mock_butt().contract_hash,
                mock_butt().address,
            )
            .unwrap()]
        );
    }

    #[test]
    fn test_update_config() {
        let (_init_result, mut deps) = init_helper(false);
        let handle_msg = HandleMsg::UpdateConfig {
            execution_fee: Uint128(MOCK_AMOUNT),
        };
        let env = mock_env(mock_user_address(), &[]);
        // = when called by a non-admin
        // = * it raises an Unauthorized error
        let handle_result = handle(&mut deps, env.clone(), handle_msg.clone());
        assert_eq!(
            handle_result.unwrap_err(),
            StdError::Unauthorized { backtrace: None }
        );

        // = when called by the admin
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        assert_eq!(config.execution_fee, mock_execution_fee());
        handle(
            &mut deps,
            mock_env(HumanAddr::from(MOCK_ADMIN), &[]),
            handle_msg,
        )
        .unwrap();
        let config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
        // = * it updates the execution_fee
        assert_eq!(config.execution_fee, Uint128(MOCK_AMOUNT))
    }
}
