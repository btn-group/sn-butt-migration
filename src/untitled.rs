#[test]
fn test_change_orders_to_processing() {
    let (_init_result, mut deps) = init_helper(true);
    let mut handle_msg = HandleMsg::ChangeOrdersToProcessing {
        order_positions: vec![],
    };

    // = when not called by an admin
    let mut handle_result = handle(
        &mut deps,
        mock_env(mock_contract().address, &[]),
        handle_msg.clone(),
    );
    // = * it raises an error
    assert_eq!(
        handle_result.unwrap_err(),
        StdError::Unauthorized { backtrace: None }
    );

    // = when called by an admin
    // == when order in order_positions does not exist
    handle_msg = HandleMsg::ChangeOrdersToProcessing {
        order_positions: vec![
            FillDetail {
                position: cosmwasm_std::Uint128(0),
                azero_transaction_hash: azero_transaction_hash.clone(),
            },
            FillDetail {
                position: cosmwasm_std::Uint128(1),
                azero_transaction_hash: azero_transaction_hash,
            },
        ],
    };
    handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
    // == * it raises an error
    assert_eq!(
        handle_result.unwrap_err(),
        NotFound {
            kind: "cw_secret_network_butt_migration::state::Order".to_string(),
            backtrace: None
        }
    );

    // == when order in order_positions exists
    create_order_helper(&mut deps);
    create_order_helper(&mut deps);
    // === when order in order_positions is processing (1)
    // ==== when order in order_positions does not have an execution fee
    let mut creator_order = order_at_position(
        &mut deps.storage,
        &deps.api.canonical_address(&mock_user_address()).unwrap(),
        1,
    )
    .unwrap();
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

    // ==== * it does not set the order status to filled
    // ==== * it does not send that order's butt to mount doom
    // ==== * it does not increase butt sent to mount doom in config by that order's amount
    handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
    creator_order = order_at_position(
        &mut deps.storage,
        &deps.api.canonical_address(&mock_user_address()).unwrap(),
        1,
    )
    .unwrap();
    let mut contract_order = order_at_position(
        &mut deps.storage,
        &deps
            .api
            .canonical_address(&mock_contract().address)
            .unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(creator_order.status, 1);
    assert_eq!(contract_order.status, 1);
    let mut handle_result_unwrapped = handle_result.unwrap();
    let mut config: Config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    assert_eq!(handle_result_unwrapped.messages, vec![]);
    assert_eq!(config.total_sent_to_mount_doom, Uint128(0));

    // ==== when order in order_positions has an execution fee
    creator_order.execution_fee = Some(cosmwasm_std::Uint128(1));
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

    // ==== * it sets the order status to filled (2) for both user and contract
    // ==== * it sends butt to mount doom
    // ==== * it increases butt sent to mount doom in config
    handle_result = handle(&mut deps, mock_env(MOCK_ADMIN, &[]), handle_msg.clone());
    creator_order = order_at_position(
        &mut deps.storage,
        &deps.api.canonical_address(&mock_user_address()).unwrap(),
        1,
    )
    .unwrap();
    contract_order = order_at_position(
        &mut deps.storage,
        &deps
            .api
            .canonical_address(&mock_contract().address)
            .unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(creator_order.status, 2);
    assert_eq!(contract_order.status, 2);
    handle_result_unwrapped = handle_result.unwrap();
    assert_eq!(
        handle_result_unwrapped.messages,
        vec![snip20::transfer_msg(
            config.mount_doom.address.clone(),
            creator_order.amount,
            None,
            BLOCK_SIZE,
            config.butt.contract_hash.clone(),
            config.butt.address.clone(),
        )
        .unwrap()]
    );
    config = TypedStore::attach(&deps.storage).load(CONFIG_KEY).unwrap();
    assert_eq!(config.total_sent_to_mount_doom, contract_order.amount);
}
