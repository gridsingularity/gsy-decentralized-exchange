# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-06T13:36:06.472Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
- **marketControllerProxy**: 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
- **orderRegistryProxy**: 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
- **tradeSettlementProxy**: 0xa513E6E4b8f2a923D98304ec87F64353C4D5C853

## Totals

- Deployment gas: 7368107
- Deployment fee: 0.005976429802286228 ETH
- Role setup gas: 339770
- Role setup fee: 0.00011658531987501 ETH
- Mutating call gas: 8211260
- Mutating call fee: 0.000316662017220006 ETH
- Deployment + setup + mutating fee: 0.006409677139381244 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 885275 | 981513999 | 0.000868909805464725 | `0x3331f00bc7ea090f677d8eac3e18433fe3d278de539e0beb39e1c36d70ba6806` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696148 | 866065664 | 0.000602909879862272 | `0x6edae9a47888ad068b50ec7bef0c9d89194f3d5998ecee5a9478ad53c6e39f3c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1478841 | 762831706 | 0.001128106802932746 | `0x327a763c200c6a9b8b133b35daa4392a4be944346dc9ae8908436b481ec633ad` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741598 | 676878633 | 0.000501971840475534 | `0x1d08f4207d093f55952fd759857b60774379735436b2c2c812922cbd200ca12b` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1495059 | 596451903 | 0.000891730785647277 | `0xd22f9bdaf1fb14f35e20eb011ce2dc1d1cdda9c2bd9e6c9bd70ad88468ba1986` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718872 | 529326506 | 0.000380518004021232 | `0x92f9f6f98d534a983f285c9ad0431a6357cf37fd9a2c15066c98cead6385df89` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 466331677 | 0.000026410694526895 | `0x80b53f678ffbe758be66abf148cb4cd231df552002875f5a7496a7a53900bb47` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 408260306 | 0.000023122230690616 | `0xe615f627f00dbab5fddbaf9c59bad7cafbff2f066dffb32f84c172aafa249030` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 357420454 | 0.000020242864832744 | `0x1c947bb328409632cbc0430dbac9f661b74ad294e99a56a5159279b1ef3d9a6b` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 312911588 | 0.000017722060697968 | `0x27c9e05ce848e9f3e83d11cc70af2cd3b5f1b72b140648622e30e5913a17677f` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 273945324 | 0.000015515167370064 | `0x7e84a2346a1fe8897b3bc122b323e1ee1fe6118ed7f650be039936ae9987824c` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 239831453 | 0.000013572301756723 | `0x4d9d70b497a81f31528bc5ce866857539c9c5e5767ddbf754ba8d4ba6e74438b` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 209965624 | 0.000011312527889872 | `0x3c1a833e670c4105fc81a3e25d25989a4448d6096d0bcdc50daf0e43112b5a28` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 183814193 | 0.000009903541090454 | `0xc8f380f1bfd81c256e1cfa6a9e6cfaace8d9f526c10dbbfc5050c115e2f5d20d` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 160919949 | 0.000008702711761869 | `0xd61c35f495f7fe9a36633e6ceaa96e135e02eaadf50179e9d0543535c890294e` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 140877479 | 0.000004531887621951 | `0x1283fa6a95fec532f625bdaef8796df13f838ab2cee90a740553098cf9085e8c` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 123305560 | 0.00000663063318344 | `0x37fc5ae235c61a57bf695e08865829cdb1268634753858d76b62f1f6e698fdfe` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 107947621 | 0.000003439427100302 | `0x1ae60a6cc395895e67d8e9ad641f5542106b43acb0077f10fc6ad31ead19a062` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | createMarkets(1 market) | 83354 | 94482830 | 0.00000787552181182 | `0x6db8f6f77b81e0d4a00921423f9af02aa1c385518bbde0129eabbe5a3dce1562` |  |
| Mutating calls | MarketController | createMarkets(3 markets) | 191471 | 82738106 | 0.000015841947893926 | `0x3ecd51755bbc2c5d64ce2fb81cd5dfe78f77dbd91b908dfa836c994e0cfd7e92` | One community's spot, flex and settlement markets for one delivery slot. |
| Mutating calls | MarketController | createMarkets(3 markets, all existing) | 41784 | 72527860 | 0.00000303050410224 | `0x7157890c36f0ab43a73935e84e96693f488fa896ee9701a3f7590a2b17cebcc2` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | MarketController | createMarkets(50 markets) | 2732427 | 63487133 | 0.000173473956361791 | `0x18622c8772e00e7c02dbb805028dd35ac1789e4a7dd7003c99d0657dd3ac9ec3` |  |
| Mutating calls | MarketController | createMarkets(50 markets, all existing) | 237724 | 56996858 | 0.000013549521071192 | `0x76a685e6b5bcb9c860520129281ce818b6cf0d7bab7e315cf49d9a4f47f846b0` | Existing markets are skipped; the cost of resending a window. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 161179 | 49985164 | 0.000008056558748356 | `0x31f48f369ac331d9e7466424228fadfeec655884b609f25cb83ee67b5b1ba8e7` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 161167 | 43804157 | 0.000007059784571219 | `0x4bfa408da23c6eb44a8fa9c35027e55f5e3b778e2f50a98b65f3d283a938bd89` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 161179 | 38387469 | 0.000006187253865951 | `0x1a6a88197cdcc21002a75f4567881db493579c5d589fdbe3fc6a339b8f307d8a` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51555 | 33640597 | 0.000001734340978335 | `0xa5fec0107eabf85478a09e016ee38db6cae2b180baae3b9a91f874f01ae82b26` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 161179 | 29449976 | 0.000004746717681704 | `0x1be65d79b07a998abb8e47375c1558a24c9dbf756033ac28295cd885fa219b03` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35950 | 25808285 | 0.00000092780784575 | `0xd2e5cd9cf55f1202b6b9376919a03c33dde12a9bede2a6f210de2d97b5a15983` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 161179 | 22589982 | 0.000003641030708778 | `0x71da53dc10a11e91d4064e50c22727b081ff55186f5287024ffeabaf2b0c6d2e` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 161167 | 19796577 | 0.000003190554925359 | `0x6e3b0a60e18aa6ee2950ade5492730c541d2ef66853e6547028c5e3ad82c08a6` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 161167 | 17348593 | 0.000002796020688031 | `0x4cfd307ec2158733a52ab004add95aad3d4b68cf1e80254cbc1d8a2f1187fb98` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 161167 | 15203320 | 0.00000245027347444 | `0xde4584f5caae91e3e76ea0945e1c0104cb7407a04a28c908b283f762a7572d5f` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 161179 | 13323324 | 0.000002147440038996 | `0xa6a306992a1b52bf84a1685988485d4e5b145ce9544d5104698d62a22ddccc39` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 161167 | 11675804 | 0.000001881754303268 | `0xd68ef474ecee9677de6d4bbf4d018e94d4dc457704c25c88e87fba9b0faac9ed` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 161179 | 10232011 | 0.000001649185300969 | `0xbdff91ffbefc972779c00bad1b5728508f303500f18aaa7ab677457ed7cae6a0` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 161167 | 8966753 | 0.000001445144680751 | `0x00a64394debc3e2b407085d3b370565cfde44b9681d86b37023d765504fcdd7a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 161179 | 7857953 | 0.000001266537006587 | `0x22d493e1bf77e6c05dba96eeef09191e228dded4318bb2790346cf32d97139a3` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 161155 | 6886264 | 0.00000110975587492 | `0xbd858669ff3224fd67f1515e22cb9fd8e7e559b24b2f0c7b73d7b4840742a78a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 161179 | 6034730 | 0.00000097267174667 | `0x7aea1003368e8f891ca9901b275d0d5d6aa04ee85fd1a0b19727812415e8faa9` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 161167 | 5288495 | 0.000000852330873665 | `0x94f808977cdde35af74a434c15213c1eaf3b8ce499edd56ff65c57881edd5ba0` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 161179 | 4634536 | 0.000000746989877944 | `0xe63b331ab2eaf91c0ba137a80fbe4095b157503efd61e95686134ea7d5516104` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 161167 | 4061445 | 0.000000654570906315 | `0x0017334a4d31565327ab520c1f0fa287b2a5b356ba7e214531b6cff349d3d0e4` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 225659 | 3559219 | 0.000000803169800321 | `0x90a7ac7ff75e35ae9971433f9af16095a1da7bfb606ebf89db0b1e0fd282714c` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 405414 | 3121011 | 0.000001265301553554 | `0xf71c3fa7e996904930b4a97cf31fa4df7ae18eef988ce0254edbda483c1656ce` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 944794 | 2741429 | 0.000002590085670626 | `0xc908b6f4dacb2dd1047a5a0174dd2d42ad49d33b6f1c3a9eda08bf7c748c2f7c` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 2420335 | 0.00000019455620864 | `0x4337940e3a788df13600ebc3967bb619c3c60569d7c151b9fe92278274c8f58d` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 31285 |  |  | view estimate |  |
| View estimates | MarketController | marketsExist(3 markets) | 36192 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29062 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 39145 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28914 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28893 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Mainnet/Volta values depend on live gas price at execution time.
