# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-06-23T09:24:30.261Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **actorRegistryProxy**: 0x9A676e781A523b5d0C0e43731313A708CB607508
- **marketControllerProxy**: 0x959922bE3CAee4b8Cd9a407cc3ac1C251C2007B1
- **orderRegistryProxy**: 0x68B1D87F95878fE05B998F19b66F4baba5De1aed
- **tradeSettlementProxy**: 0xc6e7DF5E7b4f2A278906862b61205850344D4e7d

## Totals

- Deployment gas: 6426643
- Deployment fee: 0.000975940910012945 ETH
- Role setup gas: 339770
- Role setup fee: 0.000021699232178914 ETH
- Mutating call gas: 1285607
- Mutating call fee: 0.000017687167725697 ETH
- Deployment + setup + mutating fee: 0.001015327309917556 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 237662000 | 0.00015007523483 | `0x47eba958cae4b6854a20b340fbe65567e0987f246e5c653883305666f5c72ad1` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 209204877 | 0.000150805126380573 | `0x0085e1296c1c546f1a8e5b381a0b99a50925db95e9813aa39b2e5f77c328efae` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 514984 | 184310977 | 0.000094917204179368 | `0xfe517f76e687a438f1e80c2e3dcaa800ba00b7f2ef3bf45ec552b36126cbb987` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696102 | 162063082 | 0.000112812435506364 | `0x3f439e8388b1bf85ed78ea955194f0e1c06e84a6dcf5071bc0c4d5616bf8168d` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1129738 | 142745301 | 0.000161264790861138 | `0xecfa1fffc8561112561b8bd06673d3e64a307b5732c0dde61ead347cbc9a5a91` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741564 | 126246012 | 0.000093619497642768 | `0xa2dc8f91f0803d68dd7271aca9921b631ddbc799d7041190cc64bcd292129344` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1273091 | 111245424 | 0.000141625548085584 | `0xd37b6900db300ce6ebe172de6a0ed489b5daabb5beb9be281198504a821c3b39` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718850 | 98519959 | 0.00007082107252715 | `0x46bdfad5a740edd2dc03396fca1edb1745b8b48505084d5d702079386d692f79` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 86795141 | 0.000004915642810535 | `0xec5130268a1c1c19b093206303de5e96ab5c47050ae095d8ab4f140843aa4ded` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 75986712 | 0.000004303583420832 | `0x34102fa5b0002fe0cf1dff4ca3cb5262339be4eacd2851ccc60f87b30e28e21c` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 66524236 | 0.000003767666630096 | `0x525df8f5327d094e14ce55d63ceae6e23a19cb07900120db1c054e2a7b929f17` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 58240104 | 0.000003298486530144 | `0x2cac0b374a0f074247852187250323b831269356639af2f8ce81643f4da6fc16` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 50987580 | 0.00000288773258088 | `0xbcec7999e7d3e91a56fc349ddc077244af50cddedc045bf03450a086e6b5f94c` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 44638197 | 0.000002526120206427 | `0xd25d940ab7ff70724622557d6dc0374484412d6c8af6d978ab3ef7fff1756939` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 39079475 | 0.00000210552395405 | `0x41b14b196414f9e13542adc3c9a1d2427351afd39a50729737e7f9b1ff2ca527` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 34212087 | 0.000001843278823386 | `0xee53420f45f41778a9bcebb54abcd3c0d1ba0eb12a3b8a610369571e7b92e182` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 29950938 | 0.000001619776677978 | `0x2ef982e0cb1515d77defb417aa5459b0166d39ebee03d8f86f3ca87b7c2672f5` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 26220569 | 0.000000843489484161 | `0x024314769989dcc5fdf078a1a19ea699640e0dce32f8c8140f669b79482a2aac` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 22950027 | 0.000001234114751898 | `0x28a0a84ca3386505927ad467d8ab4cce6f8c0a01aa19016908a42af0c6aee79a` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 20091558 | 0.000000640157220996 | `0x19929b97ce3dcf225c188a9ab57d607d18497f233654a00cd38e5f839ea68a16` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53014 | 17585449 | 0.000000932274993286 | `0xddf7748656395282669ccb825bbb8407d6dff89dd907095ee85710f3bebb833d` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31102 | 15395037 | 0.000000478816440774 | `0x0a1c3f4112be167aad0975c2085f9fdcbd2519dc87a5819a020f0981a027f8b2` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53014 | 13474648 | 0.000000714344989072 | `0xc8668e2daa4f48a8dab414acc29642e3c1bc4150be5e7b5685f53e93036da0d2` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 150917 | 11796271 | 0.000001780257830507 | `0x6f91cbb404f0e832acdfbf31a34b41c63af20d5ca456b0aedc36f3fa926b1320` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 150905 | 10336573 | 0.000001559840548565 | `0xafe6255dd4fbdeae24f1572a84747bd862ef1921e4a6dfc5983e2782c741fd99` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 150917 | 9057501 | 0.000001366930878417 | `0xc582e9a045879aba7697aad15811193ac58b0e8b03618d50e3adfdfc3b05b8a7` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 50833 | 7936705 | 0.000000403446525265 | `0xee6652d2a59edd3282cc7b82c9952e84dc520d3e214d26a555d57e5e50a564fc` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 150917 | 6947980 | 0.00000104856829766 | `0x230e8c714b41df3219f624c549dd81b0bcbd1d48db8a24f21ec1b778fb452d9a` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 6088220 | 0.00000021883497968 | `0x8ec7d3d5c782531759701b5149614c2f9da2c87ecd1fdffe7f3165fc468681c9` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 98018 | 5329017 | 0.000000522339588306 | `0x4189451a615e7420d8b34ab05563b2e47db152132aa02adbcee6964c7ad65f21` |  |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 4667244 | 0.000000375171741696 | `0xc137d2603573c6687a087c2cc240779797f901db6dd99c154381375fdc31dd0c` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 28991 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29033 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 34173 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28893 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- Mainnet/Volta values depend on live gas price at execution time.
