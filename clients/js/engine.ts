/* This is free and unencumbered software released into the public domain. */

import nearAPI from 'near-api-js';

abstract class Assignable {
  constructor(properties: any) {
    if (properties) {
      Object.keys(properties).map((key: any) => {
        (this as any)[key] = properties[key];
      });
    }
  }

  encode(): Uint8Array {
    return nearAPI.utils.serialize.serialize(SCHEMA, this);
  }
}

export class NewCallArgs extends Assignable {
  constructor(
      public chainID: Uint8Array,
      public ownerID: string,
      public bridgeProverID: string,
      public upgradeDelayBlocks: number) {
    super(null);
  }
}

const SCHEMA = new Map([
  [NewCallArgs, {kind: 'struct', fields: [
    ['chainID', [32]],
    ['ownerID', 'string'],
    ['bridgeProverID', 'string'],
    ['upgradeDelayBlocks', 'u64'],
  ]}],
]);
