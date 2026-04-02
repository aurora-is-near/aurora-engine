use crate::exports;

pub fn bls12381_p1_sum(input: &[u8]) -> Result<[u8; 96], u64> {
    unsafe {
        const REGISTER_ID: u64 = 1;
        let error_code =
            exports::bls12381_p1_sum(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        if error_code != 0 {
            return Err(error_code);
        }
        let mut bytes = [0u8; 96];
        exports::read_register(REGISTER_ID, bytes.as_mut_ptr() as u64);
        Ok(bytes)
    }
}

pub fn bls12381_p2_sum(input: &[u8]) -> Result<[u8; 192], u64> {
    unsafe {
        const REGISTER_ID: u64 = 1;
        let error_code =
            exports::bls12381_p1_sum(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        if error_code != 0 {
            return Err(error_code);
        }
        let mut bytes = [0u8; 192];
        exports::read_register(REGISTER_ID, bytes.as_mut_ptr() as u64);
        Ok(bytes)
    }
}

pub fn bls12381_g1_multiexp(input: &[u8]) -> [u8; 96] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_g1_multiexp(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let mut bytes = [0u8; 96];
        exports::read_register(REGISTER_ID, bytes.as_mut_ptr() as u64);
        bytes
    }
}

pub fn bls12381_g2_multiexp(input: &[u8]) -> [u8; 192] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_g2_multiexp(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let mut bytes = [0u8; 192];
        exports::read_register(REGISTER_ID, bytes.as_mut_ptr() as u64);
        bytes
    }
}

pub fn bls12381_map_fp_to_g1(input: &[u8]) -> [u8; 96] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_map_fp_to_g1(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let mut bytes = [0u8; 96];
        exports::read_register(REGISTER_ID, bytes.as_mut_ptr() as u64);
        bytes
    }
}

pub fn bls12381_map_fp2_to_g2(input: &[u8]) -> [u8; 192] {
    unsafe {
        const REGISTER_ID: u64 = 1;
        exports::bls12381_map_fp2_to_g2(input.len() as u64, input.as_ptr() as u64, REGISTER_ID);
        let mut bytes = [0u8; 192];
        exports::read_register(REGISTER_ID, bytes.as_mut_ptr() as u64);
        bytes
    }
}

pub fn bls12381_pairing_check(input: &[u8]) -> u64 {
    unsafe { exports::bls12381_pairing_check(input.len() as u64, input.as_ptr() as u64) }
}
