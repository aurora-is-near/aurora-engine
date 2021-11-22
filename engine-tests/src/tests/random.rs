use crate::test_utils;
use crate::test_utils::random::{Random, RandomConstructor};
use aurora_engine_types::U256;

#[test]
fn test_random_number_precompile() {
    let random_seed = vec![7; 32];
    let mut signer = test_utils::Signer::random();
    let mut runner = test_utils::deploy_evm().with_random_seed(random_seed.clone());

    let random_ctr = RandomConstructor::load();
    let nonce = signer.use_nonce();
    let random: Random = runner
        .deploy_contract(&signer.secret_key, |ctr| ctr.deploy(nonce), random_ctr)
        .into();

    let counter_value = random.random_256(&mut runner, &mut signer);
    assert_eq!(counter_value, Some(U256::from(random_seed.as_slice())));
}
