# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-06-23T09:19:30.687Z
- **network**: volta (73799)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: VT
- **actorRegistryProxy**: 0xA67e2cF025A8d4e1d829508a1b14398FeE03D595
- **marketControllerProxy**: 0x66f052d8581FE14FB0Cc682e3C9C341a271b0396
- **orderRegistryProxy**: 0x65A1351E51ecB9f7C5905139Fc83665fbCec431D
- **tradeSettlementProxy**: 0x762eA242D160CC943d11fFF1373663E33D67c7c3

## Totals

- Deployment gas: 6424345
- Deployment fee: 0.019273035044970415 VT
- Role setup gas: 317533
- Role setup fee: 0.000952599002222731 VT
- Mutating call gas: 1285199
- Mutating call fee: 0.003855597008996393 VT
- Deployment + setup + mutating fee: 0.024081231056189539 VT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (VT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 3000000007 | 0.001893873004419037 | `0xacb24e35216b208dbc465b828f8e6d8eae391c824926bb3bdb626ea732e2be6a` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720527 | 3000000007 | 0.002161581005043689 | `0x34a5a7c630a106d87419324707b0f1c03fedb9c9555a8ce6626ac4230a05c11b` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 514844 | 3000000007 | 0.001544532003603908 | `0x98415ed6625f916de63103d27c0bd93d7050c29a842ec234747078416d11502b` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695768 | 3000000007 | 0.002087304004870376 | `0x0a1ef603705053524c1ac833f2420603eebff84ce6f55ce7735b0e662d85b388` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1129420 | 3000000007 | 0.00338826000790594 | `0xf018949f2cbab7de25db1ef885ff0d3bb66a554dae59d116b0fbab126ff09841` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741238 | 3000000007 | 0.002223714005188666 | `0xb727fd4a1660b9a14acd668bbfc972bcba4fdf36a4141a95cf3d2b8959e88b6c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1272731 | 3000000007 | 0.003818193008909117 | `0x3a0939f5c648d2ab7f694e1308e1890f5a4bda7a2260d7e820eebb77b3ec7c2c` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718526 | 3000000007 | 0.002155578005029682 | `0xbb676c06ce44555a117ff0eeb052bdef214d0f07c36239338acc366b8a95733b` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 3000000007 | 0.000169905000396445 | `0x6c6831dd8c3499f52e0ca1c5d8d3ea889c06b2bdc9e144c38f4de75fe899def7` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 3000000007 | 0.000169908000396452 | `0xace1bd4c30d85f1cc74d23141a4014354c8f529b0a394afc2e00d8a17ad767fc` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 3000000007 | 0.000169908000396452 | `0xb6560d1e1b2671dc83a79bcf5bdf4ec7b983ca02a09c7a75afcb99c0256d85e9` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 3000000007 | 0.000169908000396452 | `0x88623da01ad74bf1323997acc4c40a22cb9f1c4043a6f4f6c5849b44429db1ad` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 3000000007 | 0.000169908000396452 | `0x07aa57a0079611502e32e6ac2d20a4bddb13ba23ec8064829a2510910ae38eed` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 3000000007 | 0.000103062000240478 | `0x4ffdcb1967b0312fa958193a591f52709f8b02ab2fe3bb8777b8fcc699e0a5e3` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 3000000007 | 0.000161634000377146 | `0x6d2054d72cb26993b15f2e3238144defb233421c6f3cea8156ad8866bd0333fa` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 3000000007 | 0.000161634000377146 | `0x13fa0f9e148e08e17a50e43c154951588eeb382c124f435708f0f72b28f8c661` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 3000000007 | 0.000162243000378567 | `0x16d7df643e1d5b873ef2056f0cc68bb753ac0539c17f577a67ca40880a43b0ac` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 3000000007 | 0.000096507000225183 | `0xe5cd3eefc94abc083d368da26d9868871c91129847c2b2d181ff46147a5ccbea` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 3000000007 | 0.00016071000037499 | `0x972cabf55cdf3af3b7ad3ca11b5689240c2dde4657def4555856efb5a0f14474` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 3000000007 | 0.000094974000221606 | `0x770a1b84f6d0888e62121754363cf8ffb83b7acac4f1fec67b142d232a50aac0` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53014 | 3000000007 | 0.000159042000371098 | `0x103a5bb73707991a745812db5ac3082d82dcc929752079a41160343393ae8a77` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31102 | 3000000007 | 0.000093306000217714 | `0x6225f72f4ac3580977dd0bb29e11210d75e19d65f3cfec41e208bcdba0169a34` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53014 | 3000000007 | 0.000159042000371098 | `0xdf495b646272d98fc2f11b0b32cf985e65ae137a18b129cc691348be4a9a16a5` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 150917 | 3000000007 | 0.000452751001056419 | `0x3bfa050062b06bbcda9cb75340012d3113f08dd630219144c43e1407a77b461b` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 150905 | 3000000007 | 0.000452715001056335 | `0xada9434ff421bc3bb42cbbceaada58581f0a6f0a2a7d5cf401d870aa328fce67` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 150917 | 3000000007 | 0.000452751001056419 | `0x5e3325f6ba369e975c0a537a7929c4cb8d17a6d143d55a6b6a2f881e8015f561` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 50833 | 3000000007 | 0.000152499000355831 | `0x1067cbc04614357e99b35d333f296b8bed02c82fe5959616689c453d6179374e` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 150917 | 3000000007 | 0.000452751001056419 | `0xc0fe99b56545c0ec86075434575b3e7b32453732110994cbbd227318413dc06d` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 3000000007 | 0.000107832000251608 | `0x7b878db6d106c3e1792b62f7272a5665f1efec0793b19db915e1a43c49bf543b` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 98018 | 3000000007 | 0.000294054000686126 | `0x88be1d8dd35fc8411b817d940bf5f2395b6c76180e3bbc003fe28b347a909e5c` |  |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 3000000007 | 0.000241152000562688 | `0xeb2f243266bdcbccd61b75061e3507f72d2297e4eb2c4a10763649e138652f58` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29278 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 28991 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29033 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 34173 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28893 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- Mainnet/Volta values depend on live gas price at execution time.
