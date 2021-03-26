/* This is free and unencumbered software released into the public domain. */

import BN from 'bn.js';
import nearAPI from 'near-api-js';

abstract class Assignable {
  abstract functionName(): string;

  encode(): Uint8Array {
    return nearAPI.utils.serialize.serialize(SCHEMA, this);
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

// Borsh-encoded parameters for the `new` function.
export class NewCallArgs extends Assignable {
  constructor(
      public chainID: Uint8Array,
      public ownerID: string,
      public bridgeProverID: string,
      public upgradeDelayBlocks: number) {
    super();
  }

  functionName(): string {
    return 'new';
  }
}

// Borsh-encoded parameters for the `meta_call` function.
export class MetaCallArgs extends Assignable {
  constructor() { // TODO
    super();
  }

  functionName(): string {
    return 'meta_call';
  }
}

// Borsh-encoded parameters for the `call` function.
export class FunctionCallArgs extends Assignable {
  constructor() { // TODO
    super();
  }

  functionName(): string {
    return 'call';
  }
}

// Borsh-encoded parameters for the `view` function.
export class ViewCallArgs extends Assignable {
  constructor() { // TODO
    super();
  }

  functionName(): string {
    return 'view';
  }
}

// Borsh-encoded parameters for the `get_storage_at` function.
export class GetStorageAtArgs extends Assignable {
  constructor() { // TODO
    super();
  }

  functionName(): string {
    return 'get_storage_at';
  }
}

// Borsh-encoded parameters for the `begin_chain` function.
export class BeginChainArgs extends Assignable {
  constructor() {
    super();
  }

  functionName(): string {
    return 'begin_chain';
  }
}

// Borsh-encoded parameters for the `begin_block` function.
export class BeginBlockArgs extends Assignable {
  constructor() { // TODO
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
  [MetaCallArgs, {kind: 'struct', fields: [
    // TODO
  ]}],
  [FunctionCallArgs, {kind: 'struct', fields: [
    // TODO
  ]}],
  [ViewCallArgs, {kind: 'struct', fields: [
    // TODO
  ]}],
  [GetStorageAtArgs, {kind: 'struct', fields: [
    // TODO
  ]}],
  [BeginChainArgs, {kind: 'struct', fields: [
    // TODO
  ]}],
  [BeginBlockArgs, {kind: 'struct', fields: [
    // TODO
  ]}],
]);
