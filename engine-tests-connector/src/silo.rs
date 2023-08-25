use crate::utils::eth::{create_eth_acc, prepare_submit_transaction, set_submit_tx};
use crate::utils::{mock_proof, TestContract};

const ETH: u128 = 10u128.pow(18);

#[tokio::test]
async fn test_silo_connector() {
    let (addr, sk) = create_eth_acc();
    let contract = TestContract::new_silo_contract().await.unwrap();
    let res = contract.add_addr_to_white_list(addr).await.unwrap();
    assert!(res.is_success());

    let amount = 5 * ETH;
    let proof = mock_proof(contract.engine_contract.id(), amount, 0, Some(addr));
    let res = contract.deposit_with_proof(&proof).await.unwrap();
    assert!(res.is_success());

    let res = contract.get_eth_balance(&addr).await.unwrap();
    assert_eq!(res, 5 * ETH);

    let (recv, _) = create_eth_acc();
    let tx_args = set_submit_tx(0, recv, ETH);
    let tx = prepare_submit_transaction(&sk, tx_args).await;
    let res = contract.submit(tx).await.unwrap();
    assert!(res.is_success());

    let res = contract.get_eth_balance(&addr).await.unwrap();
    assert_eq!(res, 3 * ETH);
    let res = contract.get_eth_balance(&recv).await.unwrap();
    assert_eq!(res, ETH);
}
