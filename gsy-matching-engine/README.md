# Matching Engine client implementation in Rust. 

Ensure that redis is running with ```brew services```.

### Run Tests (require docker).

Move to the gsy-decentralized-exchange dir
```
cd ../gsy-decentralized-exchange
```
Build the tester image
```
docker build -f gsy-matching-engine/Dockerfile.test -t gsy-matching-engine-tester .
```
Run the test container
```
docker run gsy-matching-engine-teste
```

### Run
```
gsy-matching-engine-sdk --log-level DEBUG run --setup matching_engine --run-on-redis
```

Open another terminal tab. In gsy-e repo, in a virtual environment run
```
gsy-e run -t 60s -s 60m --setup matching_engine_setup.external_matching_engine --enable-external-connection --slot-length-realtime 2s
```

In a third tab, run
```
docker run --rm --name matching_engine matching_engine web2
```

