# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-09-25T07:43:11.870Z
- **network**: volta (73799)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: VT
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0x27D29130AD371CB5c23Ac1d279732407BF49b57f
- **marketControllerProxy**: 0x5BC31A323ED35cA9FA2289A9c71434309A526A65
- **orderRegistryProxy**: 0xAdc2597129ad090B90d733b551ABa7783Eb1F3a2
- **tradeSettlementProxy**: 0xCda4e9B32BfCf2A838b83b604011f2a00E4c3004

## Totals

- Deployment gas: 6772602
- Deployment fee: 0.020317806047408214 VT
- Role setup gas: 317533
- Role setup fee: 0.000952599002222731 VT
- Mutating call gas: 4178879
- Mutating call fee: 0.012536637029252153 VT
- Deployment + setup + mutating fee: 0.033807042078883098 VT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (VT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 3000000007 | 0.001893873004419037 | `0xf30ba2d69b2aad856ce4d2627d2b3792c8aa6e8d6f4f575a26afadbe513be68f` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720527 | 3000000007 | 0.002161581005043689 | `0xea02e3c7a5d0c590576528412842c5658fcbbe37d3f88d10cfd076cb807dbbe0` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 584753 | 3000000007 | 0.001754259004093271 | `0x60ea80575a6b1eff55acfdb682c67362d9ebab5514ae2297d3d968e167e32bfe` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695757 | 3000000007 | 0.002087271004870299 | `0x7e819ab05e17939de5468d9b1372a5c2b14fd6a1e2e87d973847681a9813352e` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1307672 | 3000000007 | 0.003923016009153704 | `0xd0a539c5faa7cd97724cc8d95e6e22896c3aba837717089040731b92274ad0ff` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741260 | 3000000007 | 0.00222378000518882 | `0x7bd7f693964b1ce76142543b2bfb3c86f273b64f6ec6815d273dea5ff632d962` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1372816 | 3000000007 | 0.004118448009609712 | `0x3b7b2b52133e5d542c2c87c5952e89cee7c7d8d5255815907fc99193c91c66fa` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718526 | 3000000007 | 0.002155578005029682 | `0x3eeaabc229006bacea21deab7f70f6b2b0883544ec527a4c5abad5ed5841bc07` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 3000000007 | 0.000169905000396445 | `0x414bdb435ca04775e8a3aaccb0685820fea82367958ec30ad8501d8d94c8a3d3` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56636 | 3000000007 | 0.000169908000396452 | `0xa4cb26d3522e2780b622a9eeb03c2c4f6deb96e49ee82f5698c5e09d6a00337a` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56636 | 3000000007 | 0.000169908000396452 | `0xb02c8f65ece50ed02128659c278d441286e1ce4d131b16935d5c6626ecd3eb6e` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 3000000007 | 0.000169908000396452 | `0x58cd902bcecf168a124f8e72a244cd7a725d6203fa788da4b6db9a996f0682e9` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 3000000007 | 0.000169908000396452 | `0x20aeb50629191d3997217728cacf25955fd746c4790706c69b60f49d4447f5f6` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 3000000007 | 0.000103062000240478 | `0xd1077d64bcd1908408bdd949fedef6c5479f00fd7b92adf50ad41b79d236b16c` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 3000000007 | 0.000161634000377146 | `0xcc7520c7608fc5c4f243f41c76b0b53b1f7e67abf1e85c9ec9e618cd4d3b4160` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 3000000007 | 0.000161634000377146 | `0x1ff5c2e0827c9105fe15954a0fbd53143dca554e8716987f851ce2c9fc04cd81` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 3000000007 | 0.000162243000378567 | `0x53289a2d324ed90694f9e5f252493ba46da5e8794eaaaae7efc66093988901cc` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 3000000007 | 0.000096507000225183 | `0x56dbc48252b8156cb2262bd162af2a728657a95f2c4faceafa7b624717e82662` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 3000000007 | 0.00016071000037499 | `0xd270814421f975575d2a7787d10d964673665014167dad3dee577c0650a5fd0a` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 3000000007 | 0.000094974000221606 | `0xeac1e5c8dd6203ba289518f97b1fb855a59a39179fb27bda2cb4937b2b8d459d` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53056 | 3000000007 | 0.000159168000371392 | `0x61cf4d54ace3bde581d8f54a288488d8016bba6daf566bc86bd975fd005a106a` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31144 | 3000000007 | 0.000093432000218008 | `0x80c54cd35c8a6e84b6918ddf165a7d43e32c14dc82ac25d49251789bf73434c3` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53056 | 3000000007 | 0.000159168000371392 | `0xc9741ed51f2fa7c1f5f7259d5bcedc224052f5f60ef255077b416debb4bcff7d` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 160455 | 3000000007 | 0.000481365001123185 | `0x853f7b3f3741a5b723e158f16a470b6816be6fd4bc85dd9d8066ed4c55d1f895` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 160443 | 3000000007 | 0.000481329001123101 | `0x44ddf35612626223eae588ac626c0fc572a253f0db18f7735e084eac571ca2ad` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 160455 | 3000000007 | 0.000481365001123185 | `0x69f72b0e5de717a218272c03673763de865d1d463e7776ed0c9b4b4b8ec8eba0` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51525 | 3000000007 | 0.000154575000360675 | `0x0621e73f6f5d6e5172c92035671cf9d51ec913da5f261ff6ad7e485c29b3c4af` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 160455 | 3000000007 | 0.000481365001123185 | `0x07f6d149e58b20385726fe1eafc35a8dc90bde95420528fc01f96e49d29f5fab` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35944 | 3000000007 | 0.000107832000251608 | `0x2ce542f59414d7975545c72c69f4d825e9495a5580838c4ad9231e2412b90a89` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 160455 | 3000000007 | 0.000481365001123185 | `0xc9f6b73e4b83933112726804c034d15cb873c944a67d17c24e021634b481e0e2` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 160443 | 3000000007 | 0.000481329001123101 | `0x817ad76383633ba49ce8264dc05cd410cfc99359ab8dd77bf97e568b6432d315` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 160443 | 3000000007 | 0.000481329001123101 | `0x79c9a61272fa2ff14d888fc6990f39eb591e803aef0b9f901c928048f4cd261d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 160443 | 3000000007 | 0.000481329001123101 | `0x34e7f01a005266a50fb1c323c3bc9e5c6200719b74073de6c4178121fa92fcc3` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 160455 | 3000000007 | 0.000481365001123185 | `0x86c76ac8ccce441867c11283ee319e502ca9300679dbdc163d48c7e99d259e32` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 160443 | 3000000007 | 0.000481329001123101 | `0xe34c49499e20f53235bd4e76caa3421f8bae065b043fe25c081242f9c14c6ae3` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 160455 | 3000000007 | 0.000481365001123185 | `0xb9e56c033d1866554e19ce5e014821baa6384659571026ac1392bea285b659d2` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 160443 | 3000000007 | 0.000481329001123101 | `0xd0c8bd4e049f7777e47525db776d04a2d9d58705b441125e47bb0917fcea8948` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 160455 | 3000000007 | 0.000481365001123185 | `0x305308ab88e9e81ab9f116620032990356ec49e75aced5f5b8a8f84056c93168` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 160431 | 3000000007 | 0.000481293001123017 | `0xc2470c545d959b862f2a4a574ae3fb68146dc0d1b5ced2143bd56ac8a4c5528c` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 160455 | 3000000007 | 0.000481365001123185 | `0xc0b5c70a9a6c9c4c5a5c7496c594bc3185100ee90029efa4e3972cf083d3185a` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 160443 | 3000000007 | 0.000481329001123101 | `0xa31e04a82dc95eefb7dfc8ca56ffd18e00eb76c89298ea1e094542173d75d25a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 160455 | 3000000007 | 0.000481365001123185 | `0x80bd4395ad76405905d4ebefd2db87abe10278fee801c5103f201bb8e346b070` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 160443 | 3000000007 | 0.000481329001123101 | `0x7ef6aebbb2a0c525f73833a355e7d815dc18e767e2176ff6a5ae198fabad3887` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 113426 | 3000000007 | 0.000340278000793982 | `0x78bc9da02d0e1344345fc0f0b932c37566d779ee995aa7d601da8f7dc7f4f693` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 186641 | 3000000007 | 0.000559923001306487 | `0x12e60f19d753d090e6d3a987c1223caab9e698b8da119a77c4ec537936373420` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 406399 | 3000000007 | 0.001219197002844793 | `0xbf18bfc52adabf09b6928b3e357aa07fd7616167e45b867c3365e9e2f3f136cf` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80384 | 3000000007 | 0.000241152000562688 | `0xd651bf101b2067b8e3a50743a92b472fac3b54536d8746efaff645c9b0b00fab` |  |
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
