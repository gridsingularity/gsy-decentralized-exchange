# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-06-24T09:34:01.860Z
- **network**: ewc (246)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: EWT
- **actorRegistryProxy**: 0xFFc622d946eA79F1F701778D8319Ca3Ace96F01A
- **marketControllerProxy**: 0x0daAc33DfFF78369D32f22c2C86dB39Fe2A50899
- **orderRegistryProxy**: 0x29a319E054C10fd1878e72B1Cfb7abBd6D86fDA1
- **tradeSettlementProxy**: 0xD225AcAB790DA66CEda4bd5B7034b9a02a220F09

## Totals

- Deployment gas: 6424357
- Deployment fee: 0.00070667927 EWT
- Role setup gas: 317533
- Role setup fee: 0.00003492863 EWT
- Mutating call gas: 1285199
- Mutating call fee: 0.00014137189 EWT
- Deployment + setup + mutating fee: 0.00088297979 EWT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (EWT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 110000000 | 0.00006944201 | `0x80b47176d90947659f205724e94a646c2e31838e234d084f46109eec8e73475e` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720527 | 110000000 | 0.00007925797 | `0x6b7e60910a76cd052eeedb6c7dcdcaa38012174f72120e816ae9108fbbf69a8c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 514844 | 110000000 | 0.00005663284 | `0x15d99ca462a1675ff6eb45c4664014811571cb34ede7c88ec6de22a0726303a5` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695780 | 110000000 | 0.0000765358 | `0xa4d50ac7f7f6c13fca79443dd3137cd45fd8ed92086e793db61ff7bb8a6915c3` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1129420 | 110000000 | 0.0001242362 | `0x557b48c2740a3fcf9fa6839f119f6f29e5ad73f44d68f1b19044c3b8e0f72e7f` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741238 | 110000000 | 0.00008153618 | `0x96aa6db6c9388cb9f93f78f9f7fd9fde85f1555d55c8712b95e8467b1e3ff05c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1272731 | 110000000 | 0.00014000041 | `0x0910db177f011da3c6cb5c88759a11b924a5cf08f7f3cce3275673d277566be1` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718526 | 110000000 | 0.00007903786 | `0x150f69b731c994a7f532178ec23f506767b44cdecb499669fd3ac52cb2d61fce` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 110000000 | 0.00000622985 | `0x1a8d0a9a5b43f32fad95f80ad64cd4c37377a1516b40b5d10a909c2d7fe5cdc7` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 110000000 | 0.00000622996 | `0x74163b5c2ecae0429420c47a89bd8b91c1ba54b91c97454c86c3b9a292c71fd5` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 110000000 | 0.00000622996 | `0xb6af9b9a8bbed7325ddb9d2aa07b1a63f36bb2bd76aa4dbb8f1177464e12a791` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 110000000 | 0.00000622996 | `0x84792dd3d062a909cc5bcd4b6d86579e4acfd81513b3d561a9dc0be3fde15c49` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 110000000 | 0.00000622996 | `0x72e5438f260cd228554cc7f6737e7ce07ba3cd73f26e74900c56662e1a655be7` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 110000000 | 0.00000377894 | `0xbf022b02d02650804497f914622eabdcb7dfedcb00088f927ed092c71d547f3f` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 110000000 | 0.00000592658 | `0xbd360049f46cd2bfeb6c9174ae367fe89d52da538327823645406327d73fd00a` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 110000000 | 0.00000592658 | `0x1b49751f9dc6bd546432654762b89f5e9090069d226648c30c44b912f2816e1e` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 110000000 | 0.00000594891 | `0x1e15fe4d54e2b11b6f55d77d69708c59919bde3bbaf69134d44375c3ee3860d1` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 110000000 | 0.00000353859 | `0x6d947579ce803fa632e7dd52e07921917ca40169e7843b63503dfa92fb1351d4` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 110000000 | 0.0000058927 | `0x26b6e8e478b61215d1fe9ed7d60d1e60662c9d15524716e817d894d17b733854` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 110000000 | 0.00000348238 | `0xdfea3068a3bde115c5e7c47a75c499a556ebb3bab6219ff942fab0089310d3d8` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53014 | 110000000 | 0.00000583154 | `0x51fb2ca3379799711a3878f7cf049e10cd76d9e2d7ec1793046049c94ee93e30` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31102 | 110000000 | 0.00000342122 | `0x252c4ff223a0b707f178a7cf7a1a80361d33b490cbe5f8ed21a418483434353a` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53014 | 110000000 | 0.00000583154 | `0x555a7189f2920d79124e009cfcbc5ce9f3f164c8828cb164221f7c2e1c284b66` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 150917 | 110000000 | 0.00001660087 | `0x31247c79c69d16764b9e1af1f4da0a119bd464a4946515a8cfc3e8d9d12d0b0e` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 150905 | 110000000 | 0.00001659955 | `0x9bba7a29a1a669edd0c637e634815655b2373076dde50eee7cba0db496e47a55` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 150917 | 110000000 | 0.00001660087 | `0x3689cf7bf4e54e70c7f45cfc9f1a3fbbc18316bb0c8fdf884efd19f2c422dbcf` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 50833 | 110000000 | 0.00000559163 | `0xedc58b1cb01a15e6e41493b09cc57574297646d862f099321b6992f793c1c959` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 150917 | 110000000 | 0.00001660087 | `0x7c9ae790dbebc8cf7737153cfd5c18798cb83b8d50f86071526c24937793ca82` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 110000000 | 0.00000395384 | `0x9d6c0e533e83fdbbf2d34800cc37edfaefdfb9d487e2fa8f88e205762648484a` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 98018 | 110000000 | 0.00001078198 | `0x1b222701e881f3abed84268c26d4ac127471de972ee7fb80d421f184d1f47abd` |  |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 110000000 | 0.00000884224 | `0xf0d91056e2fe2de4125d7052599ef99b3c03465586f7a79ab9e29575471c9f62` |  |
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
