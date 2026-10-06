# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-06T10:51:13.855Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
- **marketControllerProxy**: 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
- **orderRegistryProxy**: 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
- **tradeSettlementProxy**: 0xa513E6E4b8f2a923D98304ec87F64353C4D5C853

## Totals

- Deployment gas: 7408050
- Deployment fee: 0.006016962006790209 ETH
- Role setup gas: 339770
- Role setup fee: 0.000116629299258774 ETH
- Mutating call gas: 8212070
- Mutating call fee: 0.00031679581377781 ETH
- Deployment + setup + mutating fee: 0.006450387119826793 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 925262 | 981513999 | 0.000908157605742738 | `0x47b3a189f2e689b39dcee33ae06ae04a100c4eef8860f36d749e743ccea8c830` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696104 | 866392730 | 0.00060309944492392 | `0x0611b3fd177f8f26e78463d5845360d2f400a68e159eeaa58eda1335e5945cc2` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1478841 | 763119468 | 0.001128532357176588 | `0x61ac425977bbd42dd6f3be100797bca615db6534808fac56c1a4e6f7dab4457b` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741598 | 677133971 | 0.000502161198625658 | `0x06d65288ac38bbcabd06b7e0e1813af7ac545d540535fecc70e5209a4484da0d` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1495059 | 596676901 | 0.000892067170932159 | `0x9497b08afc33252153766648cb8d975c3eeedac28b414cabfec6ad175e1dc45c` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718872 | 529526182 | 0.000380661545506704 | `0xd64649dc5e9d8176110e197ee5c30f28c417ad14977e99005093aab878900530` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 466507590 | 0.00002642065735965 | `0xecf2fd0a1da0412bbd5356ffe05bdaca41c4f19093ef95e9b464e768f8c7eb0d` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 408414314 | 0.000023130953087704 | `0xb98a63355816a756de3cfec4f2d864d034430d64417c6f8e633a9ebd45ad0124` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 357555284 | 0.000020250501064624 | `0x5837b9b84add2e7880b55682ab355d380cdadface9d8c828ea99430ad12d196c` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 313029628 | 0.000017728746011408 | `0xc98bd9b1fdcc6e87dbe5e3ea52faf5d52fee1f1466d9660a9be1fb729d477ac1` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 274048664 | 0.000015521020134304 | `0x3b33976626d606443760557d5bb83d969343dc1d0059dd7da01aa6d60ea93fe9` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 239921924 | 0.000013577421601084 | `0x9b6960d3c087e98ba2c85095691da39ff163a114250d1b033150b80dcf0fab1d` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 210044829 | 0.000011316795296862 | `0xbd2d54cf322fe119dd118f39bc50e4d4a831b02fffe31a3edc95ba8748f1c673` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 183883533 | 0.000009907276990974 | `0x2fa0e6295c9ed2f6e649a4b214bdee4283a27dc392028989cc848506a32b82f8` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 160980652 | 0.000008705994640812 | `0xb72ea64f00550e1593fb7e7f4a2d6ff81d895ea997a9b3aae3a446775598a682` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 140930621 | 0.000004533597146949 | `0x03521994b8f899171fed399d745ebff4360405a99687c251b9874a3deb52beaf` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 123352075 | 0.00000663313448105 | `0xbfd5245692640af83d0e4657b1bb2d7d0195b03fc45c769aa21a7fafc6bcca87` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 107988341 | 0.000003440724520942 | `0xe0d524e59fa616227de7f864c108f52bf7e2513933796fb2f97dbfa2d64ea79c` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | createMarkets(1 market) | 83354 | 94518472 | 0.000007878492715088 | `0xa8c58d0bfe2fc3673a401228bee9100ab3156f231937a87a2688735b84430c03` |  |
| Mutating calls | MarketController | createMarkets(3 markets) | 191471 | 82769318 | 0.000015847924086778 | `0x2a3d237c4d53ae5e3d6041152e0f550e932cdc692f5a23fd8f84c3132c10c660` | One community's spot, flex and settlement markets for one delivery slot. |
| Mutating calls | MarketController | createMarkets(3 markets, all existing) | 41784 | 72555219 | 0.000003031647270696 | `0x6e0753b4521bc600f17a3ef19176f0afac13e4b2655c7c734d5b515d22d54d75` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | MarketController | createMarkets(50 markets) | 2732427 | 63511081 | 0.000173539392523587 | `0xa3ceed20c5deb113442ca334c3ea3a667b608dbd7d080d802b9a47f0f86db698` |  |
| Mutating calls | MarketController | createMarkets(50 markets, all existing) | 237724 | 57018357 | 0.000013554631899468 | `0x4225fd96f6138be6d6853861ead2a9cc1d865de82008f5dd914c100f6f1e378b` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 161224 | 50004018 | 0.000008061847798032 | `0xc9422ef44e032e7fcd9c508f99a32d19fec63c0c5cd77a4c128de9eb9cc56af6` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 161212 | 43820698 | 0.000007064422365976 | `0x1988867889996ca92c4681df9de2f6f8c30113270b465d3d104e455a7f38c587` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 161224 | 38401982 | 0.000006191321145968 | `0x202a71a7de816067431235ce240a7b6afe05626f7cd43321bd397b3724450e88` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51555 | 33653329 | 0.000001734997376595 | `0xcd2f259d50b36345d5f5eeccaeb4358e8b1ce641f0aec35fcbf812fa4c100121` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 161224 | 29461122 | 0.000004749839933328 | `0x6342e10d5f460814f61d842d8c16250cc8d23ad48cec4a50eea6f1b9fa82ec0b` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35950 | 25818065 | 0.00000092815943675 | `0xe8d34c24932c8b9b6b687edae6faae913b4cc234db7aa4076dcd979ca53b3e4f` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 161224 | 22598541 | 0.000003643427174184 | `0x3de677e0c39454f81691f45192b8d875edc715df65cdb1a51941ca3c9296e697` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 161212 | 19804086 | 0.000003192656312232 | `0x30f6cdfb803b6a4f16e6c77226ffeda77072b1458e54e2feccfc599746b95902` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 161212 | 17355181 | 0.000002797863439372 | `0x848ee85bedcf634682ed3faef0efccf746e2504cba31ba03dca3f1964665001f` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 161212 | 15209100 | 0.0000024518894292 | `0xc84c6157ec365ad4d6d85ff5553eb5447ec846b83f9b80d38881d82420e9ea5a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 161224 | 13328395 | 0.00000214885715548 | `0x15e2a74f8b5bd24f5e08cbfbc1251cb7f5877f89cfd751df0fc0bfe0c963c416` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 161212 | 11680254 | 0.000001882997107848 | `0xd54b51b412087c9b461e3c4365c6247ccce809eee5d514f443b6adfc332352ce` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 161224 | 10235914 | 0.000001650274998736 | `0x6ee3edd6929c73822fb25e0096783bb595976de663cd79a2777f6e1872aa4ac2` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 161212 | 8970177 | 0.000001446100174524 | `0xaa4b0069200ec48355b7e415d7d47941565a181c448c1b12bd0756720fd49621` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 161224 | 7860957 | 0.000001267374931368 | `0x87650ad87e705ae7ab1d931c307d97fd4439d8d987e5d6efeb30759526e5bd08` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 161200 | 6888899 | 0.0000011104905188 | `0x7fc39ae0781abb7f1b0e9fd10a0e05a97ca52b43773f3cc602ddcaf3b462187f` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 161224 | 6037041 | 0.000000973315898184 | `0x5cbedc2a238a44e507bdb951ebd6026d8508e3844e31a85a4e9f70c66e1a5980` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 161212 | 5290522 | 0.000000852895632664 | `0xbc8640711ba501205c9833563a8680554acb14669a15dcf36c9b378bc3c1afde` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 161224 | 4636315 | 0.00000074748524956 | `0x3375af87264286e20ef0826b6c7e7663906b93787141b5fff46f061589113bce` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 161212 | 4063005 | 0.00000065500516206 | `0x5f3ed61e1f284ec78b547fe5fb03b53b23d5db0e8001e12e2839f56785d6837a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 225659 | 3560589 | 0.000000803478953151 | `0x96d9c700962cd89054fa0691c70fe00e1dfa8f6562ab5826623d0da8a33f07b9` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 405414 | 3122212 | 0.000001265788455768 | `0x6a1826425b035cbebb3f1bf8de8352c15d32c2ba192e682d6b8279a404e4b888` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 944794 | 2742484 | 0.000002591082428296 | `0xdba942a253b0784aafd64d6a2169930242e4b5662e4d94ea546a38a28fb3e0e7` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 2421267 | 0.000000194631126528 | `0x4c3db2c6f9c474a68c4eae2fdf0ab41769fd2bff9716eb68e58b360bbae01286` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 31330 |  |  | view estimate |  |
| View estimates | MarketController | marketExists(bytes16) | 28908 |  |  | view estimate |  |
| View estimates | MarketController | getMarket(bytes16) | 32396 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29062 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 39145 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28893 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Mainnet/Volta values depend on live gas price at execution time.
