use crate::utils;
use crate::utils::solidity::random::{Random, RandomConstructor};
use aurora_engine_types::H256;
use rand::SeedableRng;

#[test]
fn test_random_number_precompile() {
    let random_seed = H256::from_slice(vec![7; 32].as_slice());
    let secret_key = {
        let mut rng = rand::rngs::StdRng::from_seed(random_seed.0);
        libsecp256k1::SecretKey::random(&mut rng)
    };
    let mut signer = utils::Signer::new(secret_key);
    let mut runner = utils::deploy_runner().with_block_random_value(random_seed);

    let random_ctr = RandomConstructor::load();
    let nonce = signer.use_nonce();
    let random: Random = runner
        .deploy_contract(&signer.secret_key, |ctr| ctr.deploy(nonce), random_ctr)
        .into();

    // Value derived from `random_seed` above together with the `action_hash`
    // of the following transaction.
    let expected_value = H256::from_slice(
        &hex::decode("1a71249ace8312de8ed3640c852d5d542b04b2caec668325f6e18811244e7f5c").unwrap(),
    );
    runner.context.random_seed = expected_value.0.to_vec();

    let counter_value = random.random_seed(&mut runner, &mut signer);
    assert_eq!(counter_value, expected_value);
}
