use crate::prelude::{Address, U256};
use crate::test_utils::{
    self,
    erc20::{ERC20Constructor, ERC20},
    uniswap::{
        ExactInputParams, ExactOutputSingleParams, Factory, FactoryConstructor, MintParams, Pool,
        PositionManager, PositionManagerConstructor, SwapRouter, SwapRouterConstructor,
    },
    AuroraRunner, ExecutionProfile, Signer,
};
use aurora_engine_types::types::Wei;
use aurora_engine_types::H160;
use libsecp256k1::SecretKey;
use rand::SeedableRng;

const INITIAL_BALANCE: u64 = 1000;
const INITIAL_NONCE: u64 = 0;
// The "fee" can only be specific values, see
// https://github.com/Uniswap/uniswap-v3-core/blob/main/contracts/UniswapV3Factory.sol#L26
const POOL_FEE: u64 = 500;
const MINT_AMOUNT: u64 = 1_000_000_000_000;
const LIQUIDITY_AMOUNT: u64 = MINT_AMOUNT / 5;
const OUTPUT_AMOUNT: u64 = LIQUIDITY_AMOUNT / 100;
const INPUT_AMOUNT: u64 = LIQUIDITY_AMOUNT / 100;

#[test]
fn test_uniswap_input_multihop() {
    let mut context = UniswapTestContext::new("uniswap");

    // evm_gas = 970k
    // near total gas = 122 Tgas

    let tokens = context.create_tokens(10, MINT_AMOUNT.into());
    for (token_a, token_b) in tokens.iter().zip(tokens.iter().skip(1)) {
        context.create_pool(token_a, token_b);
        context.add_equal_liquidity(LIQUIDITY_AMOUNT.into(), token_a, token_b);
    }

    let (_amount_out, _evm_gas, profile) = context.exact_input(&tokens, INPUT_AMOUNT.into());

    assert_eq!(113, profile.all_gas() / 1_000_000_000_000);
}

#[test]
fn test_uniswap_exact_output() {
    let mut context = UniswapTestContext::new("uniswap");
    let (token_a, token_b) = context.create_token_pair(MINT_AMOUNT.into());
    let _pool = context.create_pool(&token_a, &token_b);

    let (_result, profile) =
        context.add_equal_liquidity(LIQUIDITY_AMOUNT.into(), &token_a, &token_b);
    test_utils::assert_gas_bound(profile.all_gas(), 32);
    let wasm_fraction = 100 * profile.wasm_gas() / profile.all_gas();
    assert!(
        (40..=50).contains(&wasm_fraction),
        "{}% is not between 40% and 50%",
        wasm_fraction
    );

    let (_amount_in, profile) =
        context.exact_output_single(&token_a, &token_b, OUTPUT_AMOUNT.into());
    test_utils::assert_gas_bound(profile.all_gas(), 17);
    let wasm_fraction = 100 * profile.wasm_gas() / profile.all_gas();
    assert!(
        (45..=55).contains(&wasm_fraction),
        "{}% is not between 45% and 55%",
        wasm_fraction
    );
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct LiquidityResult {
    pub token_id: U256,
    pub liquidity: U256,
    pub amount0: U256,
    pub amount1: U256,
}

pub(crate) struct UniswapTestContext {
    pub factory: Factory,
    pub manager: PositionManager,
    pub swap_router: SwapRouter,
    pub signer: Signer,
    pub runner: AuroraRunner,
    pub name: String,
}

impl UniswapTestContext {
    pub fn new(name: &str) -> Self {
        let mut runner = test_utils::deploy_evm();
        let mut rng = rand::rngs::StdRng::seed_from_u64(414243);
        let source_account = SecretKey::random(&mut rng);
        let source_address = test_utils::address_from_secret_key(&source_account);
        runner.create_address(
            source_address,
            Wei::new_u64(INITIAL_BALANCE),
            INITIAL_NONCE.into(),
        );

        let mut signer = Signer {
            nonce: INITIAL_NONCE,
            secret_key: source_account,
        };

        let nonce = signer.use_nonce();
        let factory = Factory(runner.deploy_contract(
            &signer.secret_key,
            |c| c.deploy(nonce.into()),
            FactoryConstructor::load(),
        ));

        let wrapped_eth = Self::create_token_with_runner(
            "Wrapped Ether",
            "WETH",
            U256::MAX,
            &mut runner,
            &mut signer,
        );
        let weth_address = wrapped_eth.0.address;

        let nonce = signer.use_nonce();
        let manager = PositionManager(runner.deploy_contract(
            &signer.secret_key,
            |c| {
                c.deploy(
                    factory.0.address,
                    weth_address,
                    Address::new(H160([0; 20])),
                    nonce.into(),
                )
            },
            PositionManagerConstructor::load(),
        ));

        let nonce = signer.use_nonce();
        let swap_router = SwapRouter(runner.deploy_contract(
            &signer.secret_key,
            |c| c.deploy(factory.0.address, weth_address, nonce.into()),
            SwapRouterConstructor::load(),
        ));

        Self {
            factory,
            manager,
            swap_router,
            signer,
            runner,
            name: String::from(name),
        }
    }

    pub fn no_gas(&mut self) {
        self.runner.wasm_config.regular_op_cost = 0;
    }

    pub fn create_tokens(&mut self, n: usize, mint_amount: U256) -> Vec<ERC20> {
        let names = ('a'..='z').into_iter().map(|c| format!("token_{}", c));
        let symbols = ('A'..='Z').into_iter().map(|c| format!("{}{}{}", c, c, c));
        let mut result: Vec<ERC20> = names
            .zip(symbols)
            .take(n)
            .map(|(name, symbol)| self.create_token(&name, &symbol, mint_amount))
            .collect();
        result.sort_by_key(|t| t.0.address);
        result
    }

    pub fn create_token_pair(&mut self, mint_amount: U256) -> (ERC20, ERC20) {
        let token_a = self.create_token("token_a", "A", mint_amount);
        let token_b = self.create_token("token_b", "B", mint_amount);

        if token_a.0.address < token_b.0.address {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        }
    }

    pub fn create_pool(&mut self, token_a: &ERC20, token_b: &ERC20) -> Pool {
        let token_a = token_a.0.address;
        let token_b = token_b.0.address;
        let factory = &self.factory;
        let result = self
            .runner
            .submit_with_signer(&mut self.signer, |nonce| {
                factory.create_pool(token_a, token_b, POOL_FEE.into(), nonce)
            })
            .unwrap();
        assert!(result.status.is_ok(), "Failed to create pool");

        let address = Address::try_from_slice(&test_utils::unwrap_success(result)[12..]).unwrap();
        let pool = Pool::from_address(address);

        // 2^96 corresponds to a price ratio of 1
        let result = self
            .runner
            .submit_with_signer(&mut self.signer, |nonce| {
                pool.initialize(U256::from(2).pow(U256::from(96)), nonce)
            })
            .unwrap();
        assert!(result.status.is_ok(), "Failed to initialize pool");

        pool
    }

    pub fn mint_params(&self, amount: U256, token_a: &ERC20, token_b: &ERC20) -> MintParams {
        let token0 = std::cmp::min(token_a.0.address, token_b.0.address);
        let token1 = std::cmp::max(token_a.0.address, token_b.0.address);

        MintParams {
            token0,
            token1,
            fee: POOL_FEE,
            tick_lower: -1000,
            tick_upper: 1000,
            amount0_desired: amount,
            amount1_desired: amount,
            amount0_min: U256::one(),
            amount1_min: U256::one(),
            recipient: test_utils::address_from_secret_key(&self.signer.secret_key),
            deadline: U256::MAX, // no deadline
        }
    }

    pub fn add_equal_liquidity(
        &mut self,
        amount: U256,
        token_a: &ERC20,
        token_b: &ERC20,
    ) -> (LiquidityResult, ExecutionProfile) {
        self.approve_erc20(token_a, self.manager.0.address, U256::MAX);
        self.approve_erc20(token_b, self.manager.0.address, U256::MAX);

        let params = self.mint_params(amount, token_a, token_b);

        let manager = &self.manager;
        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(&mut self.signer, |nonce| manager.mint(params, nonce))
            .unwrap();
        assert!(result.status.is_ok());

        let result = {
            let mut values = [U256::zero(); 4];
            let result_bytes = test_utils::unwrap_success(result);
            for (i, item) in values.iter_mut().enumerate() {
                let lower = i * 32;
                let upper = (i + 1) * 32;
                let value = U256::from_big_endian(&result_bytes[lower..upper]);
                *item = value;
            }
            LiquidityResult {
                token_id: values[0],
                liquidity: values[1],
                amount0: values[2],
                amount1: values[3],
            }
        };
        assert_eq!(result.amount0, amount);
        assert_eq!(result.amount1, amount);

        (result, profile)
    }

    pub fn exact_input_params(&self, amount_in: U256, token_path: &[ERC20]) -> ExactInputParams {
        let path = token_path
            .iter()
            .skip(1)
            .map(|t| (POOL_FEE, t.0.address))
            .collect();
        ExactInputParams {
            token_in: token_path[0].0.address,
            path,

            recipient: Address::new(H160([0; 20])),
            deadline: U256::MAX,
            amount_in,
            amount_out_min: U256::one(),
        }
    }

    pub fn exact_input(
        &mut self,
        token_path: &[ERC20],
        amount_in: U256,
    ) -> (U256, u64, ExecutionProfile) {
        for token in token_path.iter() {
            self.approve_erc20(token, self.swap_router.0.address, U256::MAX);
        }
        let params = self.exact_input_params(amount_in, token_path);
        let swap_router = &self.swap_router;
        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(&mut self.signer, |nonce| {
                swap_router.exact_input(params, nonce)
            })
            .unwrap();
        assert!(result.status.is_ok(), "Swap failed");

        let evm_gas = result.gas_used;
        let amount_out = U256::from_big_endian(&test_utils::unwrap_success(result));
        (amount_out, evm_gas, profile)
    }

    pub fn exact_output_single_params(
        &self,
        amount_out: U256,
        token_in: &ERC20,
        token_out: &ERC20,
    ) -> ExactOutputSingleParams {
        ExactOutputSingleParams {
            token_in: token_in.0.address,
            token_out: token_out.0.address,
            fee: POOL_FEE,

            recipient: Address::new(H160([0; 20])),
            deadline: U256::MAX,
            amount_out,
            amount_in_max: U256::from(100) * amount_out,
            price_limit: U256::zero(),
        }
    }

    pub fn exact_output_single(
        &mut self,
        token_in: &ERC20,
        token_out: &ERC20,
        amount_out: U256,
    ) -> (U256, ExecutionProfile) {
        self.approve_erc20(token_in, self.swap_router.0.address, U256::MAX);
        self.approve_erc20(token_out, self.swap_router.0.address, U256::MAX);

        let params = self.exact_output_single_params(amount_out, token_in, token_out);
        let swap_router = &self.swap_router;
        let (result, profile) = self
            .runner
            .submit_with_signer_profiled(&mut self.signer, |nonce| {
                swap_router.exact_output_single(params, nonce)
            })
            .unwrap();
        assert!(result.status.is_ok(), "Swap failed");

        let amount_in = U256::from_big_endian(&test_utils::unwrap_success(result));
        assert!(amount_in >= amount_out);

        (amount_in, profile)
    }

    pub fn approve_erc20(&mut self, token: &ERC20, spender: Address, amount: U256) {
        let result = self
            .runner
            .submit_with_signer(&mut self.signer, |nonce| {
                token.approve(spender, amount, nonce)
            })
            .unwrap();
        assert!(result.status.is_ok(), "Failed to approve ERC-20");
    }

    fn create_token(&mut self, name: &str, symbol: &str, mint_amount: U256) -> ERC20 {
        Self::create_token_with_runner(
            name,
            symbol,
            mint_amount,
            &mut self.runner,
            &mut self.signer,
        )
    }

    fn create_token_with_runner(
        name: &str,
        symbol: &str,
        mint_amount: U256,
        runner: &mut AuroraRunner,
        signer: &mut Signer,
    ) -> ERC20 {
        let nonce = signer.use_nonce();
        let contract = ERC20(runner.deploy_contract(
            &signer.secret_key,
            |c| c.deploy(name, symbol, nonce.into()),
            ERC20Constructor::load(),
        ));

        let nonce = signer.use_nonce();
        let mint_tx = contract.mint(
            test_utils::address_from_secret_key(&signer.secret_key),
            mint_amount,
            nonce.into(),
        );
        let result = runner
            .submit_transaction(&signer.secret_key, mint_tx)
            .unwrap();
        assert!(result.status.is_ok(), "Minting ERC-20 tokens failed");

        contract
    }
}
