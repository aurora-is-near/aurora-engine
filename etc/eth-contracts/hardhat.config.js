require('@nomiclabs/hardhat-waffle');
require('solidity-coverage');
require('hardhat-storage-layout');
require("./tasks/storage");

/**
 * @type import('hardhat/config').HardhatUserConfig
 */
module.exports = {
    newStorageLayoutPath: "./storageLayout",
    solidity: {
        version: '0.8.3',
        settings: {
            optimizer: {
                enabled: true,
                runs: 1000,
            },
            outputSelection: {
                "*": {
                    "*": ["storageLayout"],
                },
            },
        },
    },
};
