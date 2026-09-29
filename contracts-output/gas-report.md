# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-09-25T07:33:13.581Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
- **marketControllerProxy**: 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
- **orderRegistryProxy**: 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
- **tradeSettlementProxy**: 0xa513E6E4b8f2a923D98304ec87F64353C4D5C853

## Totals

- Deployment gas: 6774980
- Deployment fee: 0.005466040046477925 ETH
- Role setup gas: 339770
- Role setup fee: 0.000115935194865047 ETH
- Mutating call gas: 4179287
- Mutating call fee: 0.000125711389255576 ETH
- Deployment + setup + mutating fee: 0.005707686630598548 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 584915 | 981513999 | 0.000574102260725085 | `0xbe812ee0fd9c0d4800d0ca1d71dc09c2e692d638b0058641f12521c6812cf252` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696079 | 863608934 | 0.000601140043169786 | `0x8064f638f0e9d85a2602882da093409a517ba4c205860e0cf773b256c09b34b7` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1308042 | 760667319 | 0.000994984801279398 | `0x5b418804741ee8f11b96de39da47a268ef5f5844fce5aab97ebc6a26a1a625a7` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741574 | 673875444 | 0.000499728508508856 | `0x0d6fd98514f9e538c7ebc38081ba44a702c201fce936ad4f0567c714440fb43a` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1373206 | 593805418 | 0.000815417162830108 | `0xfdb5c3d107191737b6f96a8b12bab682b8c35de35b90e2e1a8dad9149ce41530` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718850 | 526374885 | 0.00037838458608225 | `0x306f14f0e84ad3921022f9c355a8fd0dcf345438585fe7b23652204b56efef62` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 463731230 | 0.00002626341821105 | `0x4d547a6e3b46f3c7a277cb477c0de8b8aaaa9ad835b245c36dddb9fc73f57e74` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 405983689 | 0.000022993292210204 | `0xb69b7649462247c465772eb968309abca4aeae77140b761cc7d1fdedce1ec663` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 355427339 | 0.000020129982771604 | `0x5d60d8f85ea96c1710868a52b8bc49d3d60187ff3cfb441003df88fcd4e0698f` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 311166672 | 0.000017623235635392 | `0x2ed131a86cc01cc0e77602f0bd4b507d62b0bc77935328068c40bdf9e593ffbf` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 272417698 | 0.000015428648743928 | `0x74ed64b57defb47d45346b342775c53185cb507e14f3ce9f9f68f776aaaa7d2f` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 238494059 | 0.000013496617292869 | `0xc33242c2e41c6e1e5e7d9c8c9b7ba87569162a96358dd6a8c43fac034aff696c` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 208794774 | 0.000011249444833572 | `0x017413b803cae193b4d372991b92a6a3523ead17bb4973f1aa9c92bea2122402` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 182789173 | 0.000009848315062894 | `0x706ccc7373ccb658ef2f0d10c67d1f7eadbc49254580ec2e1ee93aa74d4fefa4` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 160022597 | 0.000008654182068357 | `0x88cd140d5aec935d6fd48ab31fafbaf655d460c41972b1241c1a35d2652580c7` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 140091892 | 0.000004506616073748 | `0xc48f0e16fe0bfc44e5477930a0f6e98f8a0a219eeec3f198d9888be326b3d601` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 122617960 | 0.00000659365818104 | `0xa08b9a6ce6d5b8c3d396ea24eb89508aba3dd9f09c0bc515126e91407dcccf0d` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 107345663 | 0.000003420247514506 | `0x066722a02f1266e535ccbc8ab9bcf45f72fb8fff747a4660ed4b9d973ddf46df` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53056 | 93955958 | 0.000004984927307648 | `0x15e8c47f6d1c00b85de8247d2a5220992652e17a7c91cd2b5db5fd3c519081dd` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31144 | 82253005 | 0.00000256168758772 | `0x10cd9d56a85c899516b9141ae1a85ba4e02bc96ddd819bd867b236ad0c8400b5` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53056 | 71992727 | 0.000003819646123712 | `0x86be9569418236651faa40bbd11acfb377c0ff6806d78b419de6a2a7cdf931cc` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 160455 | 63025468 | 0.00001011275146794 | `0x5a4097290582570b9d913351b6d3f8657f77be8132d61876048cc33f6d691de7` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 160443 | 55231558 | 0.000008861516860194 | `0x6cf91bd095551b243af343af136e87db5a9194d9e454763ed456bdf351f31e6e` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 160455 | 48401460 | 0.0000077662562643 | `0x1757599ff0bbb96ed691d575500e66ecdcf4f8f73df68067f08bc5bb20bca444` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51525 | 42415996 | 0.0000021854841939 | `0x930a813bb7c95f48086ff4929974743b35da955aec64b6220fc413c65b4a9cf1` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 160455 | 37132209 | 0.000005958048595095 | `0x01a5148a31ae077cc5d4f4742742426acdc50ad7cc9de945e805b6b3ba27071d` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 32540335 | 0.00000116962980124 | `0x68809f785d20affe746ac182cc57e40b4978ca39adb56a34ddf890731874bdef` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 160455 | 28482541 | 0.000004570166116155 | `0x82f50971159eba7b76af672963aced7adbf1d71c7ce25aa36b8cd1bdda2909c6` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 160443 | 24960309 | 0.000004004706856887 | `0x84acad5f7ab87292b97ca0564b37f2cb4fd280d6805e08d636af8ac384321977` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 160443 | 21873643 | 0.000003509472903849 | `0x9ed5abee1468190798f40dc7d96a4d3b9f93aa7e33d86dc46e99776f6ac14681` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 160443 | 19168684 | 0.000003075481167012 | `0xa18045eb50876010f651436f76f2953231d4395febff8590eaf01db08dc72545` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 160455 | 16798228 | 0.00000269535967374 | `0x32c5a5fb20b3bd8b00516668782605842e79fff2f9c9253b4cd4700e670c8231` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 160443 | 14720910 | 0.00000236186696313 | `0xeb3fedf6a8aa2742aaa0c99b72c3e841fe145c1e7a7b27ec2fc2d34bf1ecf6c7` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 160455 | 12900480 | 0.0000020699465184 | `0xea3d343f259293702e976831373dded2b94bc215211eafd2333c3643313133f8` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 160443 | 11305170 | 0.00000181383539031 | `0x5ca882b29e1b67480bf1224fbd2277db0de163c59b24f4dfa97c422f9789c0ad` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 160455 | 9907139 | 0.000001589649988245 | `0x089afe828834bea9b9909493b497229835133f6bc3068daf4fe326a9e3db68fd` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 160431 | 8681993 | 0.000001392860818983 | `0x339387ed4b1058756746caf88a8912e6a8d53b8da560e34eba42e00e220535e4` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 160455 | 7608352 | 0.00000122079812016 | `0xe79f1d50f2b29ffbb94f2c743e1c70de40c27ca26848c49602bd86a99232f3f9` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 160443 | 6667482 | 0.000001069750814526 | `0x21698342cc762ed7493aa7bc4bed1d95b1bc4fc70edbacacd9835626850fef59` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 160455 | 5842961 | 0.000000937532307255 | `0x96a116f32cf5b1f702f9b14f892cc027a4e69af9554ca4aa8b53eb8b75e05062` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 160443 | 5120404 | 0.000000821532978972 | `0x5cda02448502e2d28e746a856f78077d4421e5b927247aa688f56d6e4702f44b` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 113426 | 4487201 | 0.000000508965260626 | `0x4d8785481b3f890ea005c2eca26b6da390230b11b78d40b29f8a676023f3839f` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 186641 | 3930542 | 0.000000733600289422 | `0x2d2067bbb58fdda1729ea534d576c3ebbb9f5c1548dde24ae4ec627e6d6498ca` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 406399 | 3445338 | 0.000001400181917862 | `0xa13c8a9f4cec53a37156ca61a6759d0216700eaae3ca06f2d9c49bf6ed3928c8` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 3026339 | 0.000000243269234176 | `0x3acb576609c635236a8bbec3b035b0fbe0f0494c23fa11850ec8e95e6f1287b8` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 28991 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29056 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 39066 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28893 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Mainnet/Volta values depend on live gas price at execution time.
