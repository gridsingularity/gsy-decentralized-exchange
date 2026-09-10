'use strict';

// One-shot bootstrap for the GSY DEX local demo.
//
// The chain starts with nobody registered, which blocks the pipeline:
//   * gsy-market-orchestrator waits until its signer is a registered *exchange
//     operator* before it will create markets, and
//   * gsy-community-client's orders go through `orderbook_registry.insert_orders`,
//     which rejects any order whose signer is not a registered *user*.
//
// Both registrations are root-only extrinsics, so we submit them via `sudo`
// signed by the dev sudo key (//Alice). Idempotent: re-running against an
// already-registered chain is a no-op.

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

// Submit `sudo.sudo(innerCall)` and resolve once it is in a block. Treats an
// "AlreadyRegistered" dispatch error as success so the script is idempotent.
function submitSudo(api, sudoPair, innerCall, label) {
  return new Promise((resolve, reject) => {
    api.tx.sudo
      .sudo(innerCall)
      .signAndSend(sudoPair, ({ status, dispatchError }) => {
        if (dispatchError) {
          let msg;
          if (dispatchError.isModule) {
            const decoded = api.registry.findMetaError(dispatchError.asModule);
            msg = `${decoded.section}.${decoded.name}`;
          } else {
            msg = dispatchError.toString();
          }
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

async function isRegistered(query, address) {
  const value = await query(address);
  return value.toHex() !== ZERO_HASH;
}

async function main() {
  const api = await connect();
  const keyring = new Keyring({ type: 'sr25519' });
  const sudo = keyring.addFromUri(SUDO_SURI);
  const operator = keyring.addFromUri(OPERATOR_SURI);
  console.log(`[bootstrap] sudo=${sudo.address} operator=${operator.address}`);

  // 1) Register trading users so `insert_orders` accepts their signed orders.
  for (const suri of USER_SURIS) {
    const pair = keyring.addFromUri(suri);
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

  console.log('[bootstrap] done.');
  await api.disconnect();
  process.exit(0);
}

main().catch((e) => {
  console.error(`[bootstrap] ERROR: ${e.message || e}`);
  process.exit(1);
});
