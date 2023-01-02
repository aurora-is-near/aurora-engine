use crate::test_types::general_state_test::GeneralStateTest;

#[test]
fn test_data_add_mod_non_const() -> Result<(), std::io::Error> {
    let cwd = std::env::current_dir().unwrap();
    println!("Current directory is {}", cwd.display());
    let data_test_add_mod_non_const = GeneralStateTest::new(
        "res/tests/GeneralStateTests/stArgsZeroOneBalance/addmodNonConst.json".to_string(),
        "addmodNonConst".to_string(),
    );
    println!("{:?}", data_test_add_mod_non_const);
    Ok(())
}
