#!/usr/bin/env bash
# Initialises the local MongoDB replica set "rs0" across the mongo1 / mongo2 / mongo3 containers.
# Idempotent: once the set is initiated, re-running is a no-op.
set -euo pipefail

PRIMARY_SEED="${MONGO_PRIMARY_SEED:-mongo1}"
USER="${MONGO_INITDB_ROOT_USERNAME:-gsy}"
PASS="${MONGO_INITDB_ROOT_PASSWORD:-gsy}"

echo "[mongo-rs-init] Waiting for ${PRIMARY_SEED} to accept authenticated connections..."
until mongosh --host "${PRIMARY_SEED}" -u "${USER}" -p "${PASS}" \
      --authenticationDatabase admin --quiet \
      --eval 'db.adminCommand("ping").ok' >/dev/null 2>&1; do
  sleep 2
done

echo "[mongo-rs-init] Initiating replica set rs0 (idempotent)..."
mongosh --host "${PRIMARY_SEED}" -u "${USER}" -p "${PASS}" \
  --authenticationDatabase admin --quiet --eval '
  try {
    rs.status();
    print("[mongo-rs-init] Replica set already initiated — nothing to do.");
  } catch (e) {
    rs.initiate({
      _id: "rs0",
      members: [
        { _id: 0, host: "mongo1:27017" },
        { _id: 1, host: "mongo2:27017" },
        { _id: 2, host: "mongo3:27017" }
      ]
    });
    print("[mongo-rs-init] Replica set rs0 initiated.");
  }
'
echo "[mongo-rs-init] Done."
