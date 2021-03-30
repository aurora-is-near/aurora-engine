/* This is free and unencumbered software released into the public domain. */
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { GetStorageAtArgs, NewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { getAddress as parseAddress } from '@ethersproject/address';
import { arrayify as parseHexString } from '@ethersproject/bytes';
import { toBigIntBE } from 'bigint-buffer';
import BN from 'bn.js';
import NEAR from 'near-api-js';
export { getAddress as parseAddress } from '@ethersproject/address';
export { arrayify as parseHexString } from '@ethersproject/bytes';
export class Engine {
    constructor(near, signer, contract) {
        this.near = near;
        this.signer = signer;
        this.contract = contract;
    }
    static connect(options, env) {
        return __awaiter(this, void 0, void 0, function* () {
            const near = yield NEAR.connect({
                deps: { keyStore: new NEAR.keyStores.InMemoryKeyStore() },
                networkId: env.NEAR_ENV || 'local',
                nodeUrl: 'http://localhost:3030',
                keyPath: `${env.HOME}/.near/validator_key.json`,
            });
            const signer = yield near.account(options.signer);
            return new Engine(near, signer, options.evm);
        });
    }
    initialize(options) {
        return __awaiter(this, void 0, void 0, function* () {
            const args = new NewCallArgs(parseHexString(defaultAbiCoder.encode(['uint256'], [options.chain || 0])), options.owner || '', options.bridgeProver || '', new BN(options.upgradeDelay || 0));
            return yield this.callMutativeFunction('new', args.encode());
        });
    }
    getVersion() {
        return __awaiter(this, void 0, void 0, function* () {
            return yield this.callFunction('get_version');
        });
    }
    getOwner() {
        return __awaiter(this, void 0, void 0, function* () {
            return yield this.callFunction('get_owner');
        });
    }
    getBridgeProvider() {
        return __awaiter(this, void 0, void 0, function* () {
            return yield this.callFunction('get_bridge_provider');
        });
    }
    getChainID() {
        return __awaiter(this, void 0, void 0, function* () {
            const result = yield this.callFunction('get_chain_id');
            return toBigIntBE(result);
        });
    }
    deployCode(bytecode) {
        return __awaiter(this, void 0, void 0, function* () {
            const args = parseHexString(bytecode);
            const result = yield this.callMutativeFunction('deploy_code', args);
            return parseAddress(result.toString('hex'));
        });
    }
    getCode(address) {
        return __awaiter(this, void 0, void 0, function* () {
            const args = parseHexString(parseAddress(address));
            return yield this.callFunction('get_code', args);
        });
    }
    getBalance(address) {
        return __awaiter(this, void 0, void 0, function* () {
            const args = parseHexString(parseAddress(address));
            const result = yield this.callFunction('get_balance', args);
            return toBigIntBE(result);
        });
    }
    getNonce(address) {
        return __awaiter(this, void 0, void 0, function* () {
            const args = parseHexString(parseAddress(address));
            const result = yield this.callFunction('get_nonce', args);
            return toBigIntBE(result);
        });
    }
    getStorageAt(address, key) {
        return __awaiter(this, void 0, void 0, function* () {
            const args = new GetStorageAtArgs(parseHexString(parseAddress(address)), parseHexString(defaultAbiCoder.encode(['uint256'], [key])));
            return yield this.callFunction('get_storage_at', args.encode());
        });
    }
    callFunction(methodName, args = null) {
        return __awaiter(this, void 0, void 0, function* () {
            const result = yield this.signer.connection.provider.query({
                request_type: 'call_function',
                account_id: this.contract,
                method_name: methodName,
                args_base64: (args ? Buffer.from(args) : Buffer.alloc(0)).toString('base64'),
                finality: 'optimistic',
            });
            if (result.logs && result.logs.length > 0)
                console.debug(result.logs); // TODO
            return Buffer.from(result.result);
        });
    }
    callMutativeFunction(methodName, args = null) {
        return __awaiter(this, void 0, void 0, function* () {
            const result = yield this.signer.functionCall(this.contract, methodName, args || Buffer.alloc(0));
            if (typeof result.status === 'object' && typeof result.status.SuccessValue === 'string') {
                return Buffer.from(result.status.SuccessValue, 'base64');
            }
            return null; // TODO: throw error
        });
    }
}
