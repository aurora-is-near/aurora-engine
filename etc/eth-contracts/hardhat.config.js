require('dotenv').config();
require('@nomiclabs/hardhat-waffle');
require('solidity-coverage');
require('./tasks/storage');
require('@nomicfoundation/hardhat-verify');

const chainId = process.env.CHAIN_ID || '1313161555';
const networkName = process.env.NETWORK_NAME || 'testnet';
const rpcUrl = process.env.RPC_URL || 'https://testnet.aurora.dev';
const apiURL = process.env.API_URL || 'https://explorer.testnet.aurora.dev/api';
const browserURL = process.env.BROWSER_URL || 'https://explorer.testnet.aurora.dev';
const privateKey = process.env.PRIVATE_KEY || '';

/**
 * @type import('hardhat/config').HardhatUserConfig
 */
module.exports = {
    newStorageLayoutPath: './storageLayout',
    solidity: {
        version: '0.8.24', // shanghai hardfork
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
    networks: {
        [networkName]: {
            url: rpcUrl,
            chainId: parseInt(chainId),
            accounts: privateKey ? [privateKey] : [],
        },
    },
    etherscan: {
        apiKey: {
            [networkName]: 'empty',
        },
        customChains: [
            {
                network: networkName,
                chainId: parseInt(chainId),
                urls: {
                    apiURL: apiURL,
                    browserURL: browserURL,
                },
            },
        ],
    },
    sourcify: {
        enabled: false,
    },
};
