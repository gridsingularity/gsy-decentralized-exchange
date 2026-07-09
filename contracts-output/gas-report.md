# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-07-02T08:00:57.820Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
- **marketControllerProxy**: 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
- **orderRegistryProxy**: 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
- **tradeSettlementProxy**: 0xa513E6E4b8f2a923D98304ec87F64353C4D5C853

## Totals

- Deployment gas: 6426631
- Deployment fee: 0.005197184482924525 ETH
- Role setup gas: 339770
- Role setup fee: 0.000115555228465478 ETH
- Mutating call gas: 3883091
- Mutating call fee: 0.000120989805746724 ETH
- Deployment + setup + mutating fee: 0.005433729517136727 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 514984 | 981513999 | 0.000505464005261016 | `0x19bb0406d07fbce4e0725197271808bb814859e979ca7e10c27300cfe4879e53` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696102 | 863036949 | 0.000600761746272798 | `0x52ae4089129c253987584082fa631fe293ee9788b96117a8e2839f95799627bc` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1129738 | 760163679 | 0.000858785794386102 | `0x54a081776b5db2fc2b5af6b25c24e318040ead6105e7faf4e58bd2d7e01304c2` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741552 | 672299768 | 0.000498545237559936 | `0x55a7c2cfb681944683fbf2bc82823fff8063de75d92baa99e5cb58273b1f5fb9` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1273091 | 592416841 | 0.000754200548525531 | `0xbfc63d2433be02e05e049f32670aea7a8168450a7fb04d089e5f13889fb29243` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718850 | 524649742 | 0.0003771444670367 | `0xd73f81872b3078c1250e9661df076f27771f1a8b10cbef129366445483955cd1` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 462211396 | 0.00002617734241246 | `0xcd0d637eeb229a2577c5f8ad9544f70706225174788affaf1b54cd634c8e27b4` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 404653116 | 0.000022917933877776 | `0x21e69989782c1cc60838ba2dcdd868ce634fb4f624187c1973e98c692f4793a2` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 354262460 | 0.00002006400868456 | `0x93f01947963fa4a148c8e10fb380baccdd3ebea3873342d586a9de6ff7f998de` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 310146853 | 0.000017565477166508 | `0x6c48090ee6f9c2134e01e00ba01ef6213c14c4c2ca7d6c510945aec9ecc1ee7f` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 271524876 | 0.000015378082877136 | `0xc69cb80afa3351185f51699b655be9f549509fb0999701a6d30bb8f92c07d00b` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 237712418 | 0.000013452383447038 | `0x53ab2e5648db81d65a438e810bd123be1804b5098880ce9af00424d6de072820` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 208110470 | 0.00001121257590266 | `0xdf99b335ebfc7fd4f9aa8a4a7a6de1b70683807150111645951b00aa6840fb70` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 182190100 | 0.0000098160382078 | `0xd038536b9b5adf8b9e397387e95fa2538a48f03686e375f13123181899d9fab8` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 159498139 | 0.000008625818855259 | `0xf760f6705c232df75bc6c5e32fb24692ed36707b1a937f7e32486b6e08083745` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 139632753 | 0.000004491846031257 | `0x1b1623ce20e102f7ad5215197d6cc397184e7daaddd895714ed22299eb0048f4` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 122216091 | 0.000006572048077434 | `0x8d587c666c01eb8d6752014a18319e8d5cbb66732e91c1f2b16139c38acc4579` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 106993847 | 0.000003409037953114 | `0xa6f2c122f68c2c9fa735321735f52e7fc94187d65fa390f7ec73b8bb302fcc32` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53014 | 93648025 | 0.00000496465639735 | `0x4da6b8241fa21091c95b93a73818db871b39dd9df3d3ae1791d7257592eefdc3` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31102 | 81983395 | 0.00000254984755129 | `0xb1dad02ba39b38e2984d61240c9f5906495dc1b0881bf8676edb9876a93c9a59` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53014 | 71756720 | 0.00000380411075408 | `0x44b0b1b5eca9344e7e65c22d65a504b1b187bc43b10f6156a168fd8eaf5d90a4` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 150917 | 62818832 | 0.000009480429668944 | `0xd63abe88c4ba1e3c7b1587f96c0c47ae41496fcec909242e1a5356ba237c05f6` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 150905 | 55045482 | 0.00000830663846121 | `0xf80ccaed48d1feb8fef4c46d0680817750d87c26a4781ac219b4821f9687d414` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 150917 | 48234020 | 0.00000727933359634 | `0xce1a7d72055723c8007e7becc2a349bc83433e16e3e04c246d12d90a6da3647d` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 50833 | 42265429 | 0.000002148478552357 | `0x98ee6ee1a88d67fe3dbc7ea57a9d5e66e58890c3343c74c20bf8921088f5e80c` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 150917 | 37000154 | 0.000005583952241218 | `0x446a330caa1ce59cf0e05a3b125b09a36baa4b0e60bca830b69c51c7e9d372da` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 32421668 | 0.000001165364434592 | `0xe92ee406a459a3f7cd793a587282dc8632c47e33368913619a2b7be913595813` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 150917 | 28378672 | 0.000004282824042224 | `0x624163dc4da03e7c749dbc6c466f6a06f6d9cc4a41c2476cc8444a9b85b04a16` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 150905 | 24867029 | 0.000003752559011245 | `0x189a4621e32a92d7577ca722446eb15207d4eb0c3c3b86c9ae5422d938b9afa1` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 150905 | 21789922 | 0.00000328820817941 | `0x7497e0e1a05d0c0d684803fc81a7a962fc7e9f523acc60d7895af15dacad7a4d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 150905 | 19093584 | 0.00000288131729352 | `0x7fb3630f4ec9c864545f106b73eeffc890317e7197a26d7a9d0e500b078be5f9` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 150917 | 16730898 | 0.000002524976933466 | `0x6a8f7ae0cf9878e6ea0acdafc96768f52e93989c64542280bde9a67766e6efdd` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 150905 | 14660577 | 0.000002212354372185 | `0x8baed87f515d26985b0a71bfaecc64da25c4cf34e8ed8904863847738d08bf2a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 150917 | 12846441 | 0.000001938746336397 | `0xbb5c38010f0062ccba9ebf8e67c018932711bcb1995cfb6ae671a10e45d59ae2` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 150905 | 11256794 | 0.00000169870649857 | `0xb6eaf74b25f1351f5568813957057a7a9c6dad02e00403ed4198a72b1bcb250a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 150917 | 9863850 | 0.00000148862265045 | `0x82a850d703e48983827b05fe40d546f9add947ff524d8134260cacd0a3a35362` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 150893 | 8643275 | 0.000001304209694575 | `0xe884ca163e0066dacc97f57b6e5aca3c72d5e34e28ba309d147754d0a938ecfe` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 150917 | 7573735 | 0.000001143005364995 | `0x49abce5cfb7206a34c4668529106254c83807cac0b7d8f561425cf12ca8aebea` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 150905 | 6636543 | 0.000001001487521415 | `0xeb7e15627cd11fbb5a32469810d77a7cb71b0f1d91ab8b464bc9d202f3e0353b` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 150917 | 5815322 | 0.000000877630950274 | `0xdeef753cc37990da36bb7437db69706430603d22fe185128088db71f119e7ab7` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 150905 | 5095721 | 0.000000768969777505 | `0xf1e781b32936f95f97ec46be6efbf490a05a49bbbaccc05b5496f65b10c84fb1` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 98018 | 4465165 | 0.00000043766654297 | `0xd598829e94ae25149d2e3ac8982d73eba4451cc2da311eeb40ee1aa9c9be827f` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 155750 | 3910667 | 0.00000060908638525 | `0xbde9285b1459cb61adef87724f87f23b062e071f5ce04579a2ec641157c952f0` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 329004 | 3426910 | 0.00000112746709764 | `0xd17fa59c679c9cd4e2941166d51d452074d196fb8163b42b0b32725af7da5a2a` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 3007942 | 0.000000241790409728 | `0x14dde7c544919cb20be87f1021b3e3937e3348fcc251dc92b49ec97bdf686e77` |  |
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
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Mainnet/Volta values depend on live gas price at execution time.
