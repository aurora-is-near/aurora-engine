const { ethers } = require('hardhat');
const { expect } = require('chai');

describe('EthCustodian contract', () => {
    let user1;
    let deployerAccount;
    let adminAccount;

    let evmErc20Factory;
    let evmErc20Contract;

    const metadataName = 'EMPTY_TOKEN';
    const metadataSymbol = 'EMPTY_SYMBOL';
    const metadataDecimals = 0;

    beforeEach(async () => {
        [deployerAccount, user1] = await ethers.getSigners();

        // Make the deployer admin
        adminAccount = deployerAccount;

        evmErc20Factory = await ethers.getContractFactory('EvmErc20');
        evmErc20Contract = await evmErc20Factory
            .connect(adminAccount)
            .deploy(
                metadataName,
                metadataSymbol,
                metadataDecimals,
                adminAccount.address,
            );
    });

    describe('AdminControlled', () => {
        it('Only admin is allowed to update the metadata', async () => {
            const newMetadataName = 'NEW_CUSTOM_TOKEN';
            const newMetadataSymbol = 'NEW_CSTM';
            const newMetadataDecimals = 18;

            await expect(
                evmErc20Contract
                    .connect(user1)
                    .setMetadata(
                        newMetadataName,
                        newMetadataSymbol,
                        newMetadataDecimals,
                    ),
            )
                .to
                .be
                .reverted;

            expect(await evmErc20Contract.name()).to.equal(metadataName);
            expect(await evmErc20Contract.symbol()).to.equal(metadataSymbol);
            expect(await evmErc20Contract.decimals()).to.equal(metadataDecimals);

            await evmErc20Contract
                .connect(adminAccount)
                .setMetadata(
                    newMetadataName,
                    newMetadataSymbol,
                    newMetadataDecimals,
                );

            expect(await evmErc20Contract.name()).to.equal(newMetadataName);
            expect(await evmErc20Contract.symbol()).to.equal(newMetadataSymbol);
            expect(await evmErc20Contract.decimals()).to.equal(newMetadataDecimals);
        });
    });

    describe('Metadata', () => {
        it('Should match the deployed metadata', async () => {
            expect(await evmErc20Contract.name()).to.equal(metadataName);
            expect(await evmErc20Contract.symbol()).to.equal(metadataSymbol);
            expect(await evmErc20Contract.decimals()).to.equal(metadataDecimals);
        });

        it('Should update the metadata', async () => {
            const newMetadataName = 'NEW_CUSTOM_TOKEN';
            const newMetadataSymbol = 'NEW_CSTM';
            const newMetadataDecimals = 18;

            await evmErc20Contract
                .connect(adminAccount)
                .setMetadata(
                    newMetadataName,
                    newMetadataSymbol,
                    newMetadataDecimals,
                );

            expect(await evmErc20Contract.name()).to.equal(newMetadataName);
            expect(await evmErc20Contract.symbol()).to.equal(newMetadataSymbol);
            expect(await evmErc20Contract.decimals()).to.equal(newMetadataDecimals);
        });
    });
});
