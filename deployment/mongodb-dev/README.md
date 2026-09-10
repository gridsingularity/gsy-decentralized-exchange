# Dev / test MongoDB replica set

The dev and test Docker Compose stacks run MongoDB as a **3-member replica set**
(`rs0`) with **keyfile internal authentication**, so local/CI runs exercise the
same replica-set connection path as production.

## How it works

Each compose file defines:

- `mongo-keyfile-init` — a one-shot init container that generates a random
  keyfile into a shared named volume (`mongo-keyfile`) with the correct
  permissions (`0400`, owned by uid/gid `999` = the `mongodb` user). Generating
  it at runtime avoids committing a keyfile, whose permissions git would not
  preserve anyway (mongod refuses a keyfile that is group/world readable).
- `mongo1`, `mongo2`, `mongo3` — the replica-set members, each started with
  `mongod --replSet rs0 --keyFile ... --bind_ip_all` and its own data volume.
  Only `mongo1` carries `MONGO_INITDB_ROOT_USERNAME/PASSWORD`, which bootstraps
  the `gsy` root user; `mongo2`/`mongo3` receive it via initial sync.
- `mongo-init` — a one-shot container that runs [`mongo-rs-init.sh`](mongo-rs-init.sh) to
  `rs.initiate()` the set (idempotent). Application services `depend_on` it with
  `condition: service_completed_successfully`, so they only start once the set
  is up.

The app connects via a seed-list URI built from these env vars (see the P0
change in `gsy-offchain-storage/src/configuration.rs`):

```
DATABASE_HOST=mongo1:27017,mongo2:27017,mongo3:27017
DATABASE_REPLICA_SET=rs0
DATABASE_AUTH_SOURCE=admin
```

which produces:

```
mongodb://gsy:gsy@mongo1:27017,mongo2:27017,mongo3:27017/?retryWrites=true&w=majority&replicaSet=rs0&authSource=admin
```
