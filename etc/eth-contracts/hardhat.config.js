require('@nomiclabs/hardhat-waffle');
require('solidity-coverage');
require('./tasks/storage');

/**
 * @type import('hardhat/config').HardhatUserConfig
 */
module.exports = {
    newStorageLayoutPath: './storageLayout',
    solidity: {
        version: '0.8.15',
        settings: {
            optimizer: {
                enabled: true,
                runs: 1000,
            },
            outputSelection: {
                '*': {
                    '*': ['storageLayout'],
                },
            },
        },
    },
};
