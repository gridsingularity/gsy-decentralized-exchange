# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-09-25T07:49:05.615Z
- **network**: ewc (246)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: EWT
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0x92d7518899Cfc743097D31C6BE958FCD2986E915
- **marketControllerProxy**: 0xA8aEc0cCBAA8c8cd2CcBD5262d3F5c76Bd700BdB
- **orderRegistryProxy**: 0x388Dd8Da1fe8BcF608b8Ab3fceE18809c37CbCb8
- **tradeSettlementProxy**: 0x9261E9Cb152F48aF6c1436cb923c1e8B07e188A2

## Totals

- Deployment gas: 6772590
- Deployment fee: 0.0007449849 EWT
- Role setup gas: 317533
- Role setup fee: 0.00003492863 EWT
- Mutating call gas: 4178459
- Mutating call fee: 0.00045963049 EWT
- Deployment + setup + mutating fee: 0.00123954402 EWT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (EWT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 110000000 | 0.00006944201 | `0x8170dde7ceb7a9709420cd2407509861be95bd26e835b9c44dcdeba4f1688f68` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720515 | 110000000 | 0.00007925665 | `0x5b5880b9608af34695ab0198c00e066066084fe685573862b9f8631ca7a16d66` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 584753 | 110000000 | 0.00006432283 | `0x2fae201f97e4b86cab81ead3cbd068d13be34356c3b650b2c29a7c569faded55` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695757 | 110000000 | 0.00007653327 | `0x652c500d90a1ae59921c3e842cf222b4e1dbf9c2761d5bc5bad66c3e7c7f9296` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1307672 | 110000000 | 0.00014384392 | `0xf1710275983cd1172dd1414dbd76ec171e3d8760ba1ae88ad8f9ddc73b22e5fa` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741260 | 110000000 | 0.0000815386 | `0x37051e58d51a5aaec2b032a49f0c657322f70a6f7ab4b588b0ecdc038d8c82ce` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1372816 | 110000000 | 0.00015100976 | `0x267dff9a50242d397b4148ebb9c07e201da381e070c13820d9f94e6730a48284` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718526 | 110000000 | 0.00007903786 | `0xf1d68016c23826ed6ed9d3e7617858c2869d9e3b1330059da78584db3a2dd9b9` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 110000000 | 0.00000622985 | `0xe9c60ad6a1bba948400e112495f85e6d4fded8cf2abd712c27a60622556b05a2` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 110000000 | 0.00000622996 | `0x23b9f231b490011ec1f8122722b4e3a98484448ae6f6251a39a7f01a57757c55` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 110000000 | 0.00000622996 | `0xae635f638eb1d48b4b670cd3d47338cb961170e3c4885408680203a5df06882d` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 110000000 | 0.00000622996 | `0x4f5e1bce7df48ac6d9dadceaa65073933062d6de2ac148dc77dd789236e8e3a6` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 110000000 | 0.00000622996 | `0x4538380307854aa8b3da9f1801bf821b6195b70670c0341688683108721da14b` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 110000000 | 0.00000377894 | `0x66e3e96c5500971bc4bb0f8d78ba84ddf951110c1b386e4e16b09f0b4a58a51a` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 110000000 | 0.00000592658 | `0x1bd6831beb07d596484cd6cdc2374e625be0fa3b8a59db7edd49281f0e96f668` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 110000000 | 0.00000592658 | `0x474c7f99cce0d4b8fc41f0f0d59fa081a7e68d294dd98e8ea8471c102b3e559b` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 110000000 | 0.00000594891 | `0x1ae08d395fe073937d89ea861a22b8a71fcc2a8e165750e80ebd4a81e9cc3378` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 110000000 | 0.00000353859 | `0x528ac616da7360b2da77bd3f85b51f9e6b7bb51450575d97ce6cca032ce10f10` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 110000000 | 0.0000058927 | `0x4351fd761bbd4b6950f702ffe159a5535a7545cdb107cf094e5b54481c1fc9b0` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 110000000 | 0.00000348238 | `0x1099714982b9252da4a8d807c95763b9e65e249078ec77f4a7b86ce844cf7a7f` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53056 | 110000000 | 0.00000583616 | `0x757df4dd92b474fe4c5ea7467f765459de38c4be54aff296c4d04fb099e02f97` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31144 | 110000000 | 0.00000342584 | `0xf2c7fd9c535739148c7a4efa4504209d5bbc001b355bdc1b8580514ae9470003` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53056 | 110000000 | 0.00000583616 | `0x80bf305eaf95c1dc3478834ef7f4bb5274b284d702ff6552fa8e604c045d47fe` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 160443 | 110000000 | 0.00001764873 | `0x0966e41e02a9aa760287f6adc335bb04e6739ac2a37fce784fd1bbfdf334dc84` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 160431 | 110000000 | 0.00001764741 | `0x02b2dd09159295569fe691fbeadbcaf703d0f5a40f2385641d1a0792d4a2d20f` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 160443 | 110000000 | 0.00001764873 | `0x6763ad6423382227c9d6d7da367eb41f429a767367d647b1268fee4b96773e10` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51513 | 110000000 | 0.00000566643 | `0x32037f278cb01f001a6c2a3e32b322dc36bf4aac401215faa025ac20dd28fa39` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 160443 | 110000000 | 0.00001764873 | `0xa3c943ec28af8cc1a589940c6a3aafef0ec20f2717279bf1950389a3066ba828` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 110000000 | 0.00000395384 | `0xcc9a03177c616274a566754a8080794634a3c8b1f35eb6f3353bfba910dc1c43` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 160443 | 110000000 | 0.00001764873 | `0x3b229d747033d49b88ec3997f8f4f29a02be5a2d27b5bcc284a54c09cda38922` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 160431 | 110000000 | 0.00001764741 | `0x7deaac7c8a39c5b26cc9225f8b175556541ac9f5d810c23690b594c06b1edb69` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 160431 | 110000000 | 0.00001764741 | `0xfa0b881d5949ae579f5a2e14e647318be09a2254c41eb263c0befc960a87e3c4` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 160431 | 110000000 | 0.00001764741 | `0xafd4798955023ee0f2679c887c1ad8a9cd0f5c68664d347172ede2feb4fbe547` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 160443 | 110000000 | 0.00001764873 | `0x94dc99171c31f538832dc59bcc3bb2b0e28af32b31c9940073e2d7a8ab90fd5f` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 160431 | 110000000 | 0.00001764741 | `0xe62ce2c389df5df8b496598fe01151c1f784d9c3a6a14547aec73141ad2acd92` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 160443 | 110000000 | 0.00001764873 | `0x57fa8efeda63414549415aa7eac2257fbb107d053b41d514c33f10f48cf66286` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 160431 | 110000000 | 0.00001764741 | `0xd20478727d3be2f145bfae01a694dbd0fa9e8b1389d5f95d692f30b7cf2ce411` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 160443 | 110000000 | 0.00001764873 | `0xb5a4be692c0b662b7d507903038dcf8227fdb1181dc47ca26ac353acebc5f3d2` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 160419 | 110000000 | 0.00001764609 | `0x52e58d18b1268645117721699424d0f2be1c3cd8e8b190479be8c994a8cdb0d6` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 160443 | 110000000 | 0.00001764873 | `0xeedccd23c40cbe78e57120c0422691418bf2cde3d11b18d20e4b2543338a2b69` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 160431 | 110000000 | 0.00001764741 | `0x93ab3cf86b50826c09522bee14ff41e34d97aa5db4dd8e9a575e3182c6a183ef` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 160443 | 110000000 | 0.00001764873 | `0x557fc9e497596e1c824dbadab127d879310d7ded82367bc9f2b362cf7024f7e8` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 160431 | 110000000 | 0.00001764741 | `0xee6f75f8d676db78b1d2f2bc87761beb62e9c2f6e3f22a7d50e1f6de8926a86f` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 113402 | 110000000 | 0.00001247422 | `0x1152b8e777537816d43cb004bb535887f0be2f761183347f3177a92eb2d00a0c` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 186593 | 110000000 | 0.00002052523 | `0xd84d7fcb25f8ffc031cd7cd3bf364ed5576aa34672254e1e2db6f97e3a5fe5c8` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 406279 | 110000000 | 0.00004469069 | `0x38ee45ec9209846529a8f9119b65e79d3949d0f8afa45bb4046c61c895c8f760` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 110000000 | 0.00000884224 | `0xa6d819265808fd99484941bcdaced74dd77c9e9a405b7f19e70b990050f983cb` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29278 |  |  | view estimate |  |
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
