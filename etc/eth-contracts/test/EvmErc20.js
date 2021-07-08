const { ethers } = require('hardhat');
const { expect } = require('chai');

describe('EthCustodian contract', () => {
    let user1;
    let adminAccount;

    let evmErc20Factory;
    let evmErc20Contract;

    const metadata_name = "EMPTY_TOKEN";
    const metadata_symbol = "EMPTY_SYMBOL";
    const metadata_decimals = 0;

    beforeEach(async () => {
        [deployerAccount, user1] = await ethers.getSigners();

        // Make the deployer admin
        adminAccount = deployerAccount;

        evmErc20Factory = await ethers.getContractFactory('EvmErc20');
        evmErc20Contract = await evmErc20Factory
            .connect(adminAccount)
            .deploy(
                metadata_name,
                metadata_symbol,
                metadata_decimals,
                adminAccount.address,
            );
    });

    describe('AdminControlled', () => {
        it('Only admin is allowed to update the metadata', async () => {
            const new_metadata_name = "NEW_CUSTOM_TOKEN";
            const new_metadata_symbol = "NEW_CSTM";
            const new_metadata_decimals = 18;

            await expect(
                evmErc20Contract
                   .connect(user1)
                   .setMetadata(
                       new_metadata_name,
                       new_metadata_symbol,
                       new_metadata_decimals,
                   )
            )
                .to
                .be
                .reverted;

            expect(await evmErc20Contract.name()).to.equal(metadata_name);
            expect(await evmErc20Contract.symbol()).to.equal(metadata_symbol);
            expect(await evmErc20Contract.decimals()).to.equal(metadata_decimals);

            await evmErc20Contract
                .connect(adminAccount)
                .setMetadata(
                    new_metadata_name,
                    new_metadata_symbol,
                    new_metadata_decimals,
                );

            expect(await evmErc20Contract.name()).to.equal(new_metadata_name);
            expect(await evmErc20Contract.symbol()).to.equal(new_metadata_symbol);
            expect(await evmErc20Contract.decimals()).to.equal(new_metadata_decimals);
        });
    });

    describe('Metadata', () => {
        it('Should match the deployed metadata', async () => {
            expect(await evmErc20Contract.name()).to.equal(metadata_name);
            expect(await evmErc20Contract.symbol()).to.equal(metadata_symbol);
            expect(await evmErc20Contract.decimals()).to.equal(metadata_decimals);
        });

        it('Should update the metadata', async() => {
            const new_metadata_name = "NEW_CUSTOM_TOKEN";
            const new_metadata_symbol = "NEW_CSTM";
            const new_metadata_decimals = 18;

            await evmErc20Contract
                .connect(adminAccount)
                .setMetadata(
                    new_metadata_name,
                    new_metadata_symbol,
                    new_metadata_decimals,
                );

            expect(await evmErc20Contract.name()).to.equal(new_metadata_name);
            expect(await evmErc20Contract.symbol()).to.equal(new_metadata_symbol);
            expect(await evmErc20Contract.decimals()).to.equal(new_metadata_decimals);
        });
    });
});
