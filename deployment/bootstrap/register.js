'use strict';

// One-shot bootstrap for the GSY DEX local demo.
//
// The chain starts with nobody registered and every vault empty, which blocks
// the pipeline:
//   * gsy-market-orchestrator waits until its signer is a registered *exchange
//     operator* before it will create markets,
//   * gsy-community-client's orders go through `orderbook_registry.insert_orders`,
//     which rejects any order whose signer is not a registered *user*, and
//   * `orderbook_worker.add_order` rejects every order with
//     `InsufficientCollateral` until the trading user's vault holds collateral.
//
// Both registrations are root-only extrinsics, so we submit them via `sudo`
// signed by the dev sudo key (//Alice); the collateral deposit is `ensure_signed`
// and is therefore signed by the trading user itself, after its registration is
// in a block. Idempotent: re-running against an already-registered chain is a
// no-op, and the deposit only ever tops the vault up to the target.

const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

const NODE_URL = process.env.NODE_URL || 'ws://gsy-node:9944';
const SUDO_SURI = process.env.SUDO_SURI || '//Alice';
// Accounts to register as trading users (comma-separated dev SURIs). The
// community client signs its orders as //Alice, so //Alice must be registered.
const USER_SURIS = (process.env.REGISTER_USER_SURIS || '//Alice')
  .split(',')
  .map((s) => s.trim())
  .filter(Boolean);
// The exchange-operator account the orchestrator signs as (defaults to sudo).
const OPERATOR_SURI = process.env.OPERATOR_SURI || SUDO_SURI;
// Collateral every trading user's vault must hold, in Planck. The default is the
// amount the e2e tests deposit right after registering
// (`e2e-tests/src/steps/common_steps.rs`). `0` disables the deposit step.
const COLLATERAL_DEPOSIT = process.env.COLLATERAL_DEPOSIT || '500000000000000';
const CONNECT_RETRIES = parseInt(process.env.CONNECT_RETRIES || '60', 10);

const ZERO_HASH = '0x' + '0'.repeat(64);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect() {
  for (let i = 1; i <= CONNECT_RETRIES; i++) {
    let api;
    try {
      const provider = new WsProvider(NODE_URL, 1000);
      api = await ApiPromise.create({ provider });
      const header = await api.rpc.chain.getHeader();
      if (header.number.toNumber() >= 1) {
        console.log(`[bootstrap] connected to ${NODE_URL} at #${header.number.toNumber()}`);
        return api;
      }
      console.log(`[bootstrap] node up but no block yet (attempt ${i}); waiting...`);
    } catch (e) {
      console.log(`[bootstrap] waiting for node (attempt ${i}/${CONNECT_RETRIES}): ${e.message || e}`);
    }
    if (api) {
      try { await api.disconnect(); } catch (_) { /* ignore */ }
    }
    await sleep(2000);
  }
  throw new Error(`could not connect to ${NODE_URL} after ${CONNECT_RETRIES} attempts`);
}

// Render a dispatch error as a readable `section.name` (or its raw form).
function describeDispatchError(api, dispatchError) {
  if (dispatchError.isModule) {
    const decoded = api.registry.findMetaError(dispatchError.asModule);
    return `${decoded.section}.${decoded.name}`;
  }
  return dispatchError.toString();
}

// Submit `sudo.sudo(innerCall)` and resolve once it is in a block. Treats an
// "AlreadyRegistered" dispatch error as success so the script is idempotent.
function submitSudo(api, sudoPair, innerCall, label) {
  return new Promise((resolve, reject) => {
    api.tx.sudo
      .sudo(innerCall)
      .signAndSend(sudoPair, ({ status, dispatchError }) => {
        if (dispatchError) {
          const msg = describeDispatchError(api, dispatchError);
          if (/AlreadyRegistered/i.test(msg)) {
            console.log(`[bootstrap] ${label}: already registered (ok)`);
            resolve();
          } else {
            reject(new Error(`${label} failed: ${msg}`));
          }
          return;
        }
        if (status.isInBlock) {
          console.log(`[bootstrap] ${label}: included in ${status.asInBlock.toHex()}`);
          resolve();
        }
      })
      .catch(reject);
  });
}

// Submit a call signed by the account itself and resolve once it is in a block.
// Unlike a `sudo` call — whose inner failure is reported through the
// `sudo.Sudid` event — a plain signed call surfaces its failure as
// `dispatchError`, so there is nothing else to inspect here.
function submitSigned(api, pair, call, label) {
  return new Promise((resolve, reject) => {
    call
      .signAndSend(pair, ({ status, dispatchError }) => {
        if (dispatchError) {
          reject(new Error(`${label} failed: ${describeDispatchError(api, dispatchError)}`));
          return;
        }
        if (status.isInBlock) {
          console.log(`[bootstrap] ${label}: included in ${status.asInBlock.toHex()}`);
          resolve();
        }
      })
      .catch(reject);
  });
}

async function isRegistered(query, address) {
  const value = await query(address);
  return value.toHex() !== ZERO_HASH;
}

// Collateral currently held by `address`, in Planck. The vault is created at
// registration, but treat a missing one as empty rather than crashing.
async function vaultCollateral(api, address) {
  const vault = await api.query.gsyCollateral.vaults(address);
  return vault.isNone ? 0n : BigInt(vault.unwrap().collateral.amount.toString());
}

// COLLATERAL_DEPOSIT must be a non-negative integer amount in Planck.
function parseCollateralTarget(raw) {
  let target;
  try {
    target = BigInt(raw.trim());
  } catch (_) {
    throw new Error(`COLLATERAL_DEPOSIT must be an integer amount in Planck, got "${raw}"`);
  }
  if (target < 0n) {
    throw new Error(`COLLATERAL_DEPOSIT must not be negative, got "${raw}"`);
  }
  return target;
}

async function main() {
  // Parse before connecting so a bad value fails immediately.
  const collateralTarget = parseCollateralTarget(COLLATERAL_DEPOSIT);
  const api = await connect();
  const keyring = new Keyring({ type: 'sr25519' });
  const sudo = keyring.addFromUri(SUDO_SURI);
  const operator = keyring.addFromUri(OPERATOR_SURI);
  const users = USER_SURIS.map((suri) => ({ suri, pair: keyring.addFromUri(suri) }));
  console.log(`[bootstrap] sudo=${sudo.address} operator=${operator.address}`);

  // 1) Register trading users so `insert_orders` accepts their signed orders.
  for (const { suri, pair } of users) {
    if (await isRegistered(api.query.gsyCollateral.registeredUser, pair.address)) {
      console.log(`[bootstrap] user ${suri} (${pair.address}) already registered; skipping`);
      continue;
    }
    console.log(`[bootstrap] registering user ${suri} (${pair.address})`);
    await submitSudo(
      api,
      sudo,
      api.tx.gsyCollateral.registerUser(pair.address),
      `register_user ${suri}`,
    );
  }

  // 2) Register the exchange operator so the orchestrator starts creating markets.
  if (await isRegistered(api.query.gsyCollateral.registeredExchangeOperator, operator.address)) {
    console.log(`[bootstrap] exchange operator ${operator.address} already registered; skipping`);
  } else {
    console.log(`[bootstrap] registering exchange operator ${operator.address}`);
    await submitSudo(
      api,
      sudo,
      api.tx.gsyCollateral.registerExchangeOperator(operator.address),
      'register_exchange_operator',
    );
  }

  // 3) Fund the trading users' vaults, or `orderbook_worker.add_order` rejects
  //    every order with `InsufficientCollateral`. `deposit_collateral` is
  //    `ensure_signed` and only accepted from a registered user, so each user
  //    signs its own deposit and this has to run after step 1. We deposit the
  //    top-up to the target, never the full amount again, so re-runs are no-ops.
  if (collateralTarget === 0n) {
    console.log('[bootstrap] COLLATERAL_DEPOSIT=0; skipping collateral deposits');
  } else {
    for (const { suri, pair } of users) {
      const current = await vaultCollateral(api, pair.address);
      if (current >= collateralTarget) {
        console.log(`[bootstrap] user ${suri} (${pair.address}) already has collateral ${current}; skipping`);
        continue;
      }
      const topUp = collateralTarget - current;
      console.log(`[bootstrap] depositing ${topUp} collateral for ${suri} (${pair.address}), vault at ${current}/${collateralTarget}`);
      await submitSigned(
        api,
        pair,
        api.tx.gsyCollateral.depositCollateral(topUp.toString()),
        `deposit_collateral ${suri} +${topUp}`,
      );
    }
  }

  console.log('[bootstrap] done.');
  await api.disconnect();
  process.exit(0);
}

main().catch((e) => {
  console.error(`[bootstrap] ERROR: ${e.message || e}`);
  process.exit(1);
});
