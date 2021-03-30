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
import { NewCallArgs } from './schema.js';
import { defaultAbiCoder } from '@ethersproject/abi';
import { arrayify } from '@ethersproject/bytes';
import NEAR from 'near-api-js';
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
            const args = new NewCallArgs(arrayify(defaultAbiCoder.encode(['uint256'], [options.chain])), options.owner, options.bridgeProver, options.upgradeDelay);
            return yield this.signer.functionCall(this.contract, 'new', args.encode());
        });
    }
    getVersion() {
        return __awaiter(this, void 0, void 0, function* () {
            return yield this.signer.viewFunction(this.contract, 'get_version', {}, { parse: (x) => x });
        });
    }
    getOwner() {
        return __awaiter(this, void 0, void 0, function* () {
            return yield this.signer.viewFunction(this.contract, 'get_owner', {}, { parse: (x) => x });
        });
    }
    getBridgeProvider() {
        return __awaiter(this, void 0, void 0, function* () {
            return yield this.signer.viewFunction(this.contract, 'get_bridge_provider', {}, { parse: (x) => x });
        });
    }
    getChainID() {
        return __awaiter(this, void 0, void 0, function* () {
            const result = yield this.signer.viewFunction(this.contract, 'get_chain_id', {}, { parse: (x) => x });
            return result.readUInt32BE(28);
        });
    }
}
