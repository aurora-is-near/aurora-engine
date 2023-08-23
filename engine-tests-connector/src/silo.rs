use crate::utils::{mock_proof, TestContract};

#[tokio::test]
async fn test_silo_connector() {
    let contract = TestContract::new_silo_contract().await.unwrap();
    let proof = mock_proof(contract.engine_contract.id(), 10, 0);
    let res = contract.deposit_with_proof(&proof).await.unwrap();
    println!("{res:#?}");
    assert!(res.is_success());
}
