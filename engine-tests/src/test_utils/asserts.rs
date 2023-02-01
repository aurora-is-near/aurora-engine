use near_primitives::transaction::ExecutionStatus;

pub fn assert_execution_status_failure(
    execution_status: ExecutionStatus,
    err_msg: &str,
    panic_msg: &str,
) {
    // Usually the converted to string has either of following two messages formats:
    // "Action #0: Smart contract panicked: ERR_MSG [src/some_file.rs:LINE_NUMBER:COLUMN_NUMBER]"
    // "right: 'MISMATCHED_DATA': ERR_MSG [src/some_file.rs:LINE_NUMBER:COLUMN_NUMBER]"
    // So the ": ERR_MSG [" pattern should catch all invariants of error, even if one of the errors
    // message is a subset of another one (e.g. `ERR_MSG_FAILED` is a subset of `ERR_MSG_FAILED_FOO`)
    let expected_err_msg_pattern = format!(": {}", err_msg);

    match execution_status {
        ExecutionStatus::Failure(err) => {
            println!("Error: {}", err);
            assert!(err.to_string().contains(&expected_err_msg_pattern));
        }
        _ => panic!("{}", panic_msg),
    }
}
