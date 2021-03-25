/* This is free and unencumbered software released into the public domain. */
import nearAPI from 'near-api-js';
class Assignable {
    constructor(properties) {
        if (properties) {
            Object.keys(properties).map((key) => {
                this[key] = properties[key];
            });
        }
    }
    encode() {
        return nearAPI.utils.serialize.serialize(SCHEMA, this);
    }
}
export class NewCallArgs extends Assignable {
    constructor(chainID, ownerID, bridgeProverID, upgradeDelayBlocks) {
        super(null);
        this.chainID = chainID;
        this.ownerID = ownerID;
        this.bridgeProverID = bridgeProverID;
        this.upgradeDelayBlocks = upgradeDelayBlocks;
    }
}
const SCHEMA = new Map([
    [NewCallArgs, { kind: 'struct', fields: [
                ['chainID', [32]],
                ['ownerID', 'string'],
                ['bridgeProverID', 'string'],
                ['upgradeDelayBlocks', 'u64'],
            ] }],
]);
