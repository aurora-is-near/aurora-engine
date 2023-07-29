use crate::utils::workspace::deploy_engine;
use aurora_engine_types::parameters::engine::{RelayerKeyArgs, RelayerKeyManagerArgs};
use aurora_engine_types::public_key::PublicKey;
use aurora_engine_types::types::Address;
use aurora_engine_workspace::parse_near;
use aurora_engine_workspace::types::{KeyType, SecretKey};
use std::fmt::Debug;
use std::str::FromStr;

#[tokio::test]
async fn test_add_key_manager() {
    let aurora = deploy_engine().await;
    let pk = PublicKey::from_str("ed25519:DcA2MzgpJbrUATQLLceocVckhhAqrkingax4oJ9kZ847").unwrap();
    let relayer_key_args = RelayerKeyArgs { public_key: pk };
    let manager = aurora
        .root()
        .create_subaccount("key_manager", parse_near!("10 N"))
        .await
        .unwrap();

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(relayer_key_args.clone())
        .deposit(parse_near!("0.5 N"))
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_failure());
    let err = result.into_result().err().unwrap();
    assert_error_message(&err, "Smart contract panicked: ERR_KEY_MANAGER_IS_NOT_SET");

    let result = aurora
        .set_key_manager(RelayerKeyManagerArgs {
            key_manager: Some(manager.id().clone()),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(relayer_key_args.clone())
        .max_gas()
        .deposit(parse_near!("0.5 N"))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = aurora
        .set_key_manager(RelayerKeyManagerArgs { key_manager: None })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(relayer_key_args)
        .max_gas()
        .deposit(parse_near!("0.5 N"))
        .transact()
        .await
        .unwrap();
    assert!(result.is_failure());
    let err = result.into_result().err().unwrap();
    assert_error_message(&err, "Smart contract panicked: ERR_KEY_MANAGER_IS_NOT_SET");
}

#[tokio::test]
async fn test_submit_by_relayer() {
    let aurora = deploy_engine().await;
    let secret_key = SecretKey::from_random(KeyType::ED25519);
    let public_key = public_key(&secret_key);
    let relayer = aurora.create_account(&aurora.id(), secret_key);

    let manager = aurora
        .root()
        .create_subaccount("key_manager", parse_near!("10 N"))
        .await
        .unwrap();
    let result = aurora
        .set_key_manager(RelayerKeyManagerArgs {
            key_manager: Some(manager.id().clone()),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let err = relayer
        .call(&aurora.id(), "submit")
        .max_gas()
        .transact()
        .await
        .err()
        .unwrap();
    assert_error_message(&err, "Failed to query access key");

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(RelayerKeyArgs { public_key })
        .max_gas()
        .deposit(parse_near!("0.5 N"))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = relayer
        .call(&aurora.id(), "submit")
        .max_gas()
        .transact()
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_relayer_key() {
    let aurora = deploy_engine().await;
    let secret_key = SecretKey::from_random(KeyType::ED25519);
    let public_key = public_key(&secret_key);
    let relayer = aurora.create_account(&aurora.id(), secret_key);

    let manager = aurora
        .root()
        .create_subaccount("key_manager", parse_near!("10 N"))
        .await
        .unwrap();
    let result = aurora
        .set_key_manager(RelayerKeyManagerArgs {
            key_manager: Some(manager.id().clone()),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(RelayerKeyArgs { public_key })
        .max_gas()
        .deposit(parse_near!("0.5 N"))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = relayer
        .call(&aurora.id(), "submit")
        .max_gas()
        .transact()
        .await;
    assert!(result.is_ok());

    let result = manager
        .call(&aurora.id(), "remove_relayer_key")
        .args_json(RelayerKeyArgs { public_key })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    // Second attempt should be finished with fail.
    let result = manager
        .call(&aurora.id(), "remove_relayer_key")
        .args_json(RelayerKeyArgs { public_key })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_failure());

    let err = relayer
        .call(&aurora.id(), "submit")
        .max_gas()
        .transact()
        .await
        .err()
        .unwrap();
    assert_error_message(&err, "unable to broadcast the transaction to the network");
}

#[tokio::test]
async fn test_call_not_allowed_method() {
    let aurora = deploy_engine().await;
    let secret_key = SecretKey::from_random(KeyType::ED25519);
    let public_key = public_key(&secret_key);
    let relayer = aurora.create_account(&aurora.id(), secret_key);

    let manager = aurora
        .root()
        .create_subaccount("key_manager", parse_near!("10 N"))
        .await
        .unwrap();
    let result = aurora
        .set_key_manager(RelayerKeyManagerArgs {
            key_manager: Some(manager.id().clone()),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(RelayerKeyArgs { public_key })
        .max_gas()
        .deposit(parse_near!("0.5 N"))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let err = relayer
        .call(&aurora.id(), "register_relayer")
        .args_borsh(Address::zero())
        .max_gas()
        .transact()
        .await
        .err()
        .unwrap();

    assert_error_message(&err, "unable to broadcast the transaction to the network");
}

#[tokio::test]
async fn test_call_not_allowed_contract() {
    let aurora = deploy_engine().await;
    let secret_key = SecretKey::from_random(KeyType::ED25519);
    let public_key = public_key(&secret_key);
    let relayer = aurora.create_account(&aurora.id(), secret_key);

    let manager = aurora
        .root()
        .create_subaccount("key_manager", parse_near!("10 N"))
        .await
        .unwrap();
    let result = aurora
        .set_key_manager(RelayerKeyManagerArgs {
            key_manager: Some(manager.id().clone()),
        })
        .max_gas()
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let result = manager
        .call(&aurora.id(), "add_relayer_key")
        .args_json(RelayerKeyArgs { public_key })
        .max_gas()
        .deposit(parse_near!("0.5 N"))
        .transact()
        .await
        .unwrap();
    assert!(result.is_success());

    let err = relayer
        .call(&"some-contract.near".parse().unwrap(), "submit")
        .args_borsh(Address::zero())
        .max_gas()
        .transact()
        .await
        .err()
        .unwrap();
    assert_error_message(&err, "unable to broadcast the transaction to the network");
}

fn public_key(sk: &SecretKey) -> PublicKey {
    let pk_str = serde_json::to_string(&sk.public_key()).unwrap();
    PublicKey::from_str(pk_str.trim_matches('"')).unwrap()
}

fn assert_error_message<T: Debug>(err: &T, expected: &str) {
    let err_msg = format!("{err:?}");
    assert!(err_msg.contains(expected));
}
