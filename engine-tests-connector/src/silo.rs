use crate::utils::eth::{create_eth_acc, set_submit_tx, submit_transaction};
use crate::utils::{mock_proof, TestContract};

const ETH: u128 = 10u128.pow(18);

#[tokio::test]
async fn test_silo_connector() {
    let (addr, sk) = create_eth_acc();
    let contract = TestContract::new_silo_contract().await.unwrap();
    let res = contract.add_addr_to_white_list(addr.clone()).await.unwrap();
    assert!(res.is_success());

    let amount = 5 * ETH;
    let proof = mock_proof(contract.engine_contract.id(), amount, 0, Some(addr));
    let res = contract.deposit_with_proof(&proof).await.unwrap();
    assert!(res.is_success());

    let res = contract.get_eth_balance(&addr).await.unwrap();
    assert_eq!(res, 5 * ETH);

    let (recv, _) = create_eth_acc();
    let tx = set_submit_tx(0, recv, 30);
    submit_transaction(contract, &sk, tx).await;
}
