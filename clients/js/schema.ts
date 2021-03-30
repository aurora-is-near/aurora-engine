/* This is free and unencumbered software released into the public domain. */

import BN from 'bn.js';
import NEAR from 'near-api-js';

abstract class Assignable {
  abstract functionName(): string;

  encode(): Uint8Array {
    return NEAR.utils.serialize.serialize(SCHEMA, this);
  }

  toFunctionCall(): object {
    return {
      methodName: this.functionName(),
      args: this.encode(),
      gas: new BN('300000000000000'),
      deposit: new BN(0),
    };
  }
}

// Borsh-encoded parameters for the `new` method.
export class NewCallArgs extends Assignable {
  constructor(
      public chainID: Uint8Array,
      public ownerID: string,
      public bridgeProverID: string,
      public upgradeDelayBlocks: number | BN) {
    super();
  }

  functionName(): string {
    return 'new';
  }
}

// Borsh-encoded parameters for the `get_chain_id` method.
export class GetChainID extends Assignable {
  constructor() { super(); }

  functionName(): string {
    return 'get_chain_id';
  }
}

// Borsh-encoded parameters for the `meta_call` method.
export class MetaCallArgs extends Assignable {
  constructor(
      public signature: Uint8Array,
      public v: number,
      public nonce: Uint8Array,
      public feeAmount: Uint8Array,
      public feeAddress: Uint8Array,
      public contractAddress: Uint8Array,
      public value: Uint8Array,
      public methodDef: string,
      public args: Uint8Array) {
    super();
  }

  functionName(): string {
    return 'meta_call';
  }
}

// Borsh-encoded parameters for the `call` method.
export class FunctionCallArgs extends Assignable {
  constructor(
      public contract: Uint8Array,
      public input: Uint8Array) {
    super();
  }

  functionName(): string {
    return 'call';
  }
}

// Borsh-encoded parameters for the `view` method.
export class ViewCallArgs extends Assignable {
  constructor(
      public sender: Uint8Array,
      public address: Uint8Array,
      public amount: Uint8Array,
      public input: Uint8Array) {
    super();
  }

  functionName(): string {
    return 'view';
  }
}

// Borsh-encoded parameters for the `get_storage_at` method.
export class GetStorageAtArgs extends Assignable {
  constructor(
      public address: Uint8Array,
      public key: Uint8Array) {
    super();
  }

  functionName(): string {
    return 'get_storage_at';
  }
}

// Borsh-encoded parameters for the `begin_chain` method.
export class BeginChainArgs extends Assignable {
  constructor(
      public chainID: Uint8Array) {
    super();
  }

  functionName(): string {
    return 'begin_chain';
  }
}

// Borsh-encoded parameters for the `begin_block` method.
export class BeginBlockArgs extends Assignable {
  constructor(
      public hash: Uint8Array,
      public coinbase: Uint8Array,
      public timestamp: Uint8Array,
      public number: Uint8Array,
      public difficulty: Uint8Array,
      public gaslimit: Uint8Array) {
    super();
  }

  functionName(): string {
    return 'begin_block';
  }
}

const SCHEMA = new Map<Function, any>([
  [NewCallArgs, {kind: 'struct', fields: [
    ['chainID', [32]],
    ['ownerID', 'string'],
    ['bridgeProverID', 'string'],
    ['upgradeDelayBlocks', 'u64'],
  ]}],
  [GetChainID, {kind: 'struct', fields: []}],
  [MetaCallArgs, {kind: 'struct', fields: [
    ['signature', [64]],
    ['v', 'u8'],
    ['nonce', [32]],
    ['feeAmount', [32]],
    ['feeAddress', [20]],
    ['contractAddress', [20]],
    ['value', [32]],
    ['methodDef', 'string'],
    ['args', ['u8']],
  ]}],
  [FunctionCallArgs, {kind: 'struct', fields: [
    ['contract', [20]],
    ['input', ['u8']],
  ]}],
  [ViewCallArgs, {kind: 'struct', fields: [
    ['sender', [20]],
    ['address', [20]],
    ['amount', [32]],
    ['input', ['u8']],
  ]}],
  [GetStorageAtArgs, {kind: 'struct', fields: [
    ['address', [20]],
    ['key', [32]],
  ]}],
  [BeginChainArgs, {kind: 'struct', fields: [
    ['chainID', [32]],
  ]}],
  [BeginBlockArgs, {kind: 'struct', fields: [
    ['hash', [32]],
    ['coinbase', [32]],
    ['timestamp', [32]],
    ['number', [32]],
    ['difficulty', [32]],
    ['gaslimit', [32]],
  ]}],
]);
