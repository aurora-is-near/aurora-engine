require('hardhat-storage-layout');
// eslint-disable-next-line no-undef
task('storageLayout', 'automatically generates the contract storage layout')
    .setAction(async () => {
        // eslint-disable-next-line no-undef
        await hre.storageLayout.export();
    });

module.exports = {};
