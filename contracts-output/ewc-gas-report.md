# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-07-02T08:10:50.631Z
- **network**: ewc (246)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: EWT
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0x89724cCAB3429fF18942905ab228A2594F5b10f1
- **marketControllerProxy**: 0x01f07dfb593573Ba5a19c5D8C807f51E56D48F1c
- **orderRegistryProxy**: 0x27D29130AD371CB5c23Ac1d279732407BF49b57f
- **tradeSettlementProxy**: 0x5BC31A323ED35cA9FA2289A9c71434309A526A65

## Totals

- Deployment gas: 6424345
- Deployment fee: 0.00070667795 EWT
- Role setup gas: 317533
- Role setup fee: 0.00003492863 EWT
- Mutating call gas: 3882659
- Mutating call fee: 0.00042709249 EWT
- Deployment + setup + mutating fee: 0.00116869907 EWT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (EWT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 110000000 | 0.00006944201 | `0x531eb77b9721035d5397dd10d9c2f480c828d7594e8b16137b2bfea2b719236b` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720515 | 110000000 | 0.00007925665 | `0x42ca45420f8a60c62ed221d8b12ed11eea66dd2dc9ddee1e10903cf4b3d8530a` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 514844 | 110000000 | 0.00005663284 | `0x556e27c00b8b55a33e61138be3a81734b03b275050670739c2326499c17fd1d3` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695780 | 110000000 | 0.0000765358 | `0xe43ec65fe414412934d225045087536d16fa4a87579a984445f5e9277d4e9c39` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1129420 | 110000000 | 0.0001242362 | `0xc0b1cbfbc3c86b068dc88b9aea0366be6cb83925029a8b9833ffc4eb42d0336a` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741238 | 110000000 | 0.00008153618 | `0x694379d390246fc5fb555a2b871fe8dfc18a28addd75d7e52157e0980fbf9617` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1272731 | 110000000 | 0.00014000041 | `0x62f8fc8ba463ba49a498bf3dec29c3f56153ca5c5e5221d19333629e5e49ccd2` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718526 | 110000000 | 0.00007903786 | `0xeae8d11213ce83b09868a1dd5964eb76ebf4ac83c4d5548b29365e0bc472a0e9` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 110000000 | 0.00000622985 | `0x76ede546b00bb55bdb4b17df8d84f970fa36e29f96cecd19fb379f4162c6ce48` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 110000000 | 0.00000622996 | `0x442d6fbef1017149c1ae1531f5b1d9988cfd9e7ddaaa8f0d7afe90fc7102d177` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 110000000 | 0.00000622996 | `0x4c999c0dbcb6475004552b7d3ae5508713f5d241b56f7c7313de55a5c3c62f8d` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 110000000 | 0.00000622996 | `0xd364d23c7daed6e9f344dbc07b6059e42d66f0dc825f78636d87ab31d3b2a939` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 110000000 | 0.00000622996 | `0xa7ff5c1749e1ab7618b09cb2e3b0bf0a9c14edbf16886fb9dc8092c272854688` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 110000000 | 0.00000377894 | `0x7804d9639cdf371bd6e6e7041535592d5c87a29253f44a6131f514f410932eaa` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 110000000 | 0.00000592658 | `0xc43ba0a43a9ec0e7fe773beb137dfc53a7005756239a2144861763a5f51df1fe` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 110000000 | 0.00000592658 | `0xfc7f57a7177ae1649567f5fd1da461c26d7538cb84f7c0d97b0a74342a7e0efe` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 110000000 | 0.00000594891 | `0x1f02d19aa4d1d1708cce2d470320702ed5813bbb3f97b856514027cf6a728259` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 110000000 | 0.00000353859 | `0x7d4fe8229beccecfac0ce1fc9b4d53d815f1f66e256412ab0b2dd0eff20c4608` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 110000000 | 0.0000058927 | `0x55baec7e25c9337ee7fc6cafeec0016b81659b30c3bd60872445fbdedb31b8db` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 110000000 | 0.00000348238 | `0x38ba76681077138c6d3fa51f7d3bc35cbc0bb41b093db8e468e9dca7533d6b62` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53014 | 110000000 | 0.00000583154 | `0x42931027a0918231f4b3513dc29f29f574c2217bf4e41005c669dcd0f73acc99` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31102 | 110000000 | 0.00000342122 | `0x3406687d6ab19b538f4ee5e6e9a32c539c14729fac66469d731a1d5886593fec` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53014 | 110000000 | 0.00000583154 | `0x83ac9057eadb53d0f61358ecb8532af32d28da9e091ec4ad069b326d4f4f8404` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 150917 | 110000000 | 0.00001660087 | `0xc3c6ca538ec8fee823b7b7ef8ad4cabea1b4b19a0150916f235c0ba8f67ed27f` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 150905 | 110000000 | 0.00001659955 | `0x7b18107b3c8642ba6015038aa0ba0b6da1adbc1fb3215094da0181784a79045c` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 150917 | 110000000 | 0.00001660087 | `0x1e4a702831ff44a97fbe2f8c8cd48a78e799234f57bb84bfffbd0b6453fd31ab` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 50833 | 110000000 | 0.00000559163 | `0xaa164c708e1d9cbfb19048f55b80898efda5619c58ad8ae2dd1a86efa5a885de` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 150917 | 110000000 | 0.00001660087 | `0xf332f43b8152e5bb5ec902ce4b3ba580f212acc1813d0b5017cb61be194a694c` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 110000000 | 0.00000395384 | `0x95f3bb363a63bef4c841d9359161d6fbedad96cfff5e65c192b05e86b4ba1a98` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 150905 | 110000000 | 0.00001659955 | `0x0b33dceee6330045590b2de810d818fc3ba0cb574e926078b4662dbf834aaa27` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 150905 | 110000000 | 0.00001659955 | `0x280edfba3597f9bde3e1178f6e288ff7b4484ae4aa9ce64b6a93dde255f384f9` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 150905 | 110000000 | 0.00001659955 | `0x726ae1436736aae7938789a0e7e8fe754f39fb1a8755e2a86ba4729574df89b0` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 150905 | 110000000 | 0.00001659955 | `0x055ceea3a7160336fc7a55c2cfca7bc2b348a950d640b6b7ee7258da3ac7543a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 150917 | 110000000 | 0.00001660087 | `0x9bba34ecbc92ed4287532656341975147a564134c132bee1862bd77aa49f1427` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 150905 | 110000000 | 0.00001659955 | `0xa17297a4b4d67979592bd4631e307bc78b9eb01a532b18ec7528f8f442ccff0a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 150917 | 110000000 | 0.00001660087 | `0x4c5898eb9150dd57b34f3bd3e8d88603e43e3e83a71a91d18a1ebe843291f8c1` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 150905 | 110000000 | 0.00001659955 | `0xcee1f06e8c232e816a740b3ae97287b61568696d8f9c3d2dcac9c808aa4f3069` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 150917 | 110000000 | 0.00001660087 | `0xb6c3a53eb2b2521a7a258e5ee39db6457484d1689e53accf01f50abb8b88313e` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 150893 | 110000000 | 0.00001659823 | `0xa464db77b7a89449437017069ae4ad86ccdce49455a78b44089ff3cd1bdb1fed` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 150917 | 110000000 | 0.00001660087 | `0x81ddf0a9761edd3c54531a85b25e03cc5bf1e4f9cce173118c9a353725b30e4a` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 150905 | 110000000 | 0.00001659955 | `0x3440008523f0826b23c9964ce9d30ae12f800cafb689e360a867d6f99dc87625` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 150917 | 110000000 | 0.00001660087 | `0x6c9c48fa7c23636fd4dd187c22d077afad8e7edfd1a89511289fbcc0f25fd98d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 150905 | 110000000 | 0.00001659955 | `0xa12ec3e2f0d0877ccf904019ed3a148196eed9ea72799848147fb8202cc54103` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 98018 | 110000000 | 0.00001078198 | `0xd451bd768619af9d49cec16fc40051b7e4775d71ebd55196c6099a4c52c2a282` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 155738 | 110000000 | 0.00001713118 | `0x7ebe8c0dcd1b0d2a7b97362ec7f37ba4256e235a19121fdc46bb66e269bbdf9f` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 329004 | 110000000 | 0.00003619044 | `0x68be9762429be0dda7887c28df48ed5e4e8898d5546087342e446a1a0b467175` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 110000000 | 0.00000884224 | `0x1dba8ad249485384df677866c7d0edc23d83b388155455e9ff45f2266c8a361a` |  |
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
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Mainnet/Volta values depend on live gas price at execution time.
