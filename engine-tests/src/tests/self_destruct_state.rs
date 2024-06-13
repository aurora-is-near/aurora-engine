use crate::utils;
use crate::utils::solidity::self_destruct::{
    SelfDestruct, SelfDestructConstructor, SelfDestructFactory, SelfDestructFactoryConstructor,
};

/// Check that account state should be properly removed after calling selfdestruct
#[test]
// For CANCUN hard fork SELFDESCTRUCT disabled
#[ignore]
fn test_self_destruct_reset_state() {
    let mut signer = utils::Signer::random();
    let mut runner = utils::deploy_runner();

    let sd_factory_ctr = SelfDestructFactoryConstructor::load();
    let nonce = signer.use_nonce();
    let sd_factory: SelfDestructFactory = runner
        .deploy_contract(&signer.secret_key, |ctr| ctr.deploy(nonce), sd_factory_ctr)
        .into();

    let sd_contract_addr = sd_factory.deploy(&mut runner, &mut signer);

    let sd: SelfDestruct = SelfDestructConstructor::load()
        .0
        .deployed_at(sd_contract_addr)
        .into();

    let counter_value = sd.counter(&mut runner, &mut signer);
    assert_eq!(counter_value, Some(0));
    sd.increase(&mut runner, &mut signer);
    let counter_value = sd.counter(&mut runner, &mut signer);
    assert_eq!(counter_value, Some(1));
    sd.finish(&mut runner);
    let counter_value = sd.counter(&mut runner, &mut signer);
    assert!(counter_value.is_some());

    let sd_contract_addr1 = sd_factory.deploy(&mut runner, &mut signer);
    assert_eq!(sd_contract_addr, sd_contract_addr1);

    let counter_value = sd.counter(&mut runner, &mut signer);
    assert_eq!(counter_value, Some(0));
}

#[test]
fn test_self_destruct_with_submit() {
    let mut signer = utils::Signer::random();
    let mut runner = utils::deploy_runner();

    let sd_factory_ctr = SelfDestructFactoryConstructor::load();
    let nonce = signer.use_nonce();
    let sd_factory: SelfDestructFactory = runner
        .deploy_contract(&signer.secret_key, |ctr| ctr.deploy(nonce), sd_factory_ctr)
        .into();

    let sd_contract_addr = sd_factory.deploy(&mut runner, &mut signer);

    let sd: SelfDestruct = SelfDestructConstructor::load()
        .0
        .deployed_at(sd_contract_addr)
        .into();

    sd.finish_using_submit(&mut runner, &mut signer);
}
