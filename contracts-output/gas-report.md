# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-15T19:23:09.055Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0xb7278A61aa25c888815aFC32Ad3cC52fF24fE575
- **marketControllerProxy**: 0x82e01223d51Eb87e16A03E24687EDF0F294da6f1
- **orderRegistryProxy**: 0x7969c5eD335650692Bc04293B07F5BF2e7A673C0
- **tradeSettlementProxy**: 0xc351628EB244ec633d5f21fBD6621e1a683B1181

## Totals

- Deployment gas: 7652739
- Deployment fee: 0.000004546033417547 ETH
- Role setup gas: 339680
- Role setup fee: 0.000000087609114714 ETH
- Mutating call gas: 7638824
- Mutating call fee: 0.000000110264735262 ETH
- Deployment + setup + mutating fee: 0.000004743907267523 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 948784 | 0.00000059912388856 | `0x1b5bb172a8089b78e45f0627db20484eb87022d00494f4b1c0db06eca75d769d` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 835179 | 0.000000602037946971 | `0xd98c4d8d0ae72843e58597bfac758715fbcaba4c3e902ee46d6912f4c25ebbb7` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 625121 | 735800 | 0.0000004599640318 | `0xba37a8523f23d406f5bbf81f71ac1f81cb38abe5d903789a3195c7a2c0639267` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696079 | 647658 | 0.000000450821132982 | `0xab1eec24eae563c58427a46c3d6095a529ec13c61c81925ad40702e4ee850a39` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1620042 | 570458 | 0.000000924165919236 | `0x6340847348ee940d2317c26616cc8eb42747ae58a72885ee4d9ad60f7e176209` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741632 | 506853 | 0.000000375898404096 | `0x849d4ed7b027ff6252571a39396d5e8ebd60c5e75c09cfd0d44473b60dd1f8ca` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1898657 | 446630 | 0.00000084799717591 | `0xafcdc84019acd36bbdd4f8dea5374911fbdaed5d02061f7e6b909698643ef753` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718894 | 397868 | 0.000000286024917992 | `0x7fe85c726831d1ec21cdc3b8113215c33d12fd16c8e0f35ffd07fad85bdad64b` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56591 | 350518 | 0.000000019836164138 | `0x2f6fca46035a249b891ed9fba9d1c2076275727b26747acf01b2274e464c5e0d` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56658 | 306869 | 0.000000017386583802 | `0xe29825d2063ca7d444551cc3ecaaef9e1974dbcc3eaeea1e98b630593ef3f3b6` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56658 | 268656 | 0.000000015221511648 | `0x25b4819e611e47c4abfa608b9c47e457442424d4573e5aa4755c53298a16c6ed` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56591 | 235202 | 0.000000013310316382 | `0xe170230efea881473cf15d40baa6e112a1a003e58a81ac6c77f82d8ef61d9d97` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56591 | 205913 | 0.000000011652822583 | `0x20943c9f9f406011e3edc2dc53d6fe4a9b397febe989a0ba38ca0cb1b5e0536b` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 180271 | 0.000000010201716161 | `0x69a51dd6ea4d714b5ae2434838ccbbaf73ae2f6dacb1ba0464b0f62d1d1d4832` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 157823 | 0.000000008503187594 | `0x750fd6756a4f39126e4ddf9a9e3a03efba0cd5e7892a0bc9f8cf79f4f2278bb8` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 138167 | 0.000000007444161626 | `0x8faac631b8a704037992b7f604d694e56fa9dd85db07ffc76d697ead25972cd6` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 120958 | 0.000000006541529598 | `0x20963d409634affcdf8a401879f7b2e092464725a11385feed47e3621bd5c3c3` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 105893 | 0.000000003406471917 | `0x07a71e0201776078aa9e9fc3e2a81a7af8c7b951dcb76ec4018e427f3d2591c1` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 92685 | 0.00000000498404319 | `0x11bb15a8d138bbe7f38ba9c4c3aff72a598b2d20e60371de2e394b6e816a8ba1` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 81141 | 0.000000002585314542 | `0x73922d244c1d18950e864634f1e9ecce58b9193872d3a9cae288c3efe36350cc` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 75605 | 71021 | 0.000000005369542705 | `0x5e9e7b395613c3bcb2ad4a4b235495c018fc0880ffbcaeb3b201b8d2d5a6d93b` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31386 | 62189 | 0.000000001951863954 | `0xf4dc3eb5328fc04ddaaf3e2e3c80f4f01cd56c5430945760d3cbb63cabad75fb` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 58505 | 54432 | 0.00000000318454416 | `0x8e289f807961840143fe574c3c28be14e3ed27a9c855df19653b35c9933440ef` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 181790 | 47654 | 0.00000000866302066 | `0x4d5fcfbcf8895ef1d8ed47d32dbf70d0a013e6a093a00937e9e829807965165a` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 181778 | 41770 | 0.00000000759286706 | `0xf54570a09db9fc8fa0cd3e7fadac4028c93c6cb1d911bf95b12bd81903d25f51` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 181790 | 36613 | 0.00000000665587727 | `0x5cb8d117ef68860bf6233e96ab123db0fb29b74fe5058eae3fceb425f2f2cf5e` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 72038 | 32092 | 0.000000002311843496 | `0xb9ce23d6da208edaa1551cb381eb9f163ad8dd4f1fd97e9618c5357a5700a29c` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 181790 | 28100 | 0.000000005108299 | `0xa10796ea6177eff05d936f4952d3740a5f2be3f0320bf07b4f2cd955a34bfe4a` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 36027 | 24631 | 0.000000000887381037 | `0x78424324e55e51dc2093c605ab17e0d59d8ffbf134e606bb8584cdcb9fc97123` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 181790 | 21561 | 0.00000000391957419 | `0x517c7e32b8f3919a506d17948f69ee822e721602398dbb588ffc30b409eaca51` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 181778 | 18899 | 0.000000003435422422 | `0xcfdf32538f96c405194a61f9f25c4c09c63250cec022ea5258211593c08e8d8c` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 181778 | 16565 | 0.00000000301115257 | `0x9bce818cff301312dd45f96550056dcdf4bb4f7569bb5b225b77564e461154eb` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 181778 | 14520 | 0.00000000263941656 | `0xf6cb71a713b4738798f93257a5e966bd1f014d21b6960efa8ea150e1a08c171c` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 181790 | 12728 | 0.00000000231382312 | `0xb4d7cbd9094d68fca8b1f69344a5fbe7769875fb0bd2d97e28ac2fcc33af7dc5` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 181778 | 11157 | 0.000000002028097146 | `0xa7d249918c4a810854ce3c1560597c8297293c16752e0f80f28817bd687be84a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 181790 | 9780 | 0.0000000017779062 | `0xb4dd126f45aa99d17991874eb84a22103e7501375adb80447eb7534e17601964` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 181778 | 8573 | 0.000000001558382794 | `0x72c123451451a42f993c20b1cca68719386b13379f88d84bde9478e1bb35e757` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 181790 | 7515 | 0.00000000136615185 | `0x75eae4363ebdb9686d652d5237e5517aeb561d136e74b8ae49d48cd314a2bbd0` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 181766 | 6587 | 0.000000001197292642 | `0x602c2518ef50819f5e151570b97c8b18a038fb99e8bcc434438082df8e346179` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 181790 | 5775 | 0.00000000104983725 | `0xbb451d56e107ee00d29f48d8c8cd54ff3b474bde4f9741ab96b9f157ee90fbf8` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 181778 | 5062 | 0.000000000920160236 | `0x362b3dd019cf836d305389b8933fcb92049902661f1e6e39a8484857251675fa` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 181790 | 4438 | 0.00000000080678402 | `0x89018f02b7b8ad696c65537335784ae4b7e72d665794f1be85a35b6c78c8f18a` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 181778 | 3890 | 0.00000000070711642 | `0xec324bfbbbed4e98b5ef273d6c53093b7536d8e4a45f8ab9b44062699092209f` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 298744 | 3410 | 0.00000000101871704 | `0x699dc8026374e154a18079f0fbb9494f2c3fbb5cc903c4be75c7b7a40c01ac97` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 538538 | 2993 | 0.000000001611844234 | `0x656734ca49f3ae98853cae7001905dead461f6ed2eb387cd1c950cb36f87af3a` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 1258030 | 2632 | 0.00000000331113496 | `0x58e6690a7c39325de31de7aefa121a78a21d68be89c65fb53efd64c287c04546` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 104515 | 2331 | 0.000000000243624465 | `0x7cd45a17917657a7d26aac2d2595b59d11740ad2a3e40dd7020bc9d413c58206` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification zero penalties bid | 181790 | 2042 | 0.00000000037121518 | `0xba309a3ff118f07bbb1b8f5775cfb60a1799958bed3b68e553df38a5ec7714b2` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification zero penalties offer | 181778 | 1790 | 0.00000000032538262 | `0x181ff0925d151cc61140e665d47d2561acd712163abeecdf1c1b2dfb16457596` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) verification zero penalties | 298744 | 1570 | 0.00000000046902808 | `0xdbe2f64f969f5e158e8611ce2a847dae9a61bb3a5e8201b2995d335fbcbddb2d` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | completeDeliveryVerification(DeliveryVerification[1]) zero penalties | 120435 | 1378 | 0.00000000016595943 | `0x6587e64467bfccf982310cfdb911375588652834347d36d689d8bff2bd9d6d33` | Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification seller penalty bid | 181790 | 1207 | 0.00000000021942053 | `0xdc58941695280cb3fe4173fb563b423e1ef03b8b90ae98e57ceb7c6ddde067fc` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification seller penalty offer | 181778 | 1059 | 0.000000000192502902 | `0xb72aadf0dce868dd3f99cfde4fbdc29baa2d97901e7a4c01af7263a051b6e398` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) verification seller penalty | 298744 | 928 | 0.000000000277234432 | `0x3f7276e1ea06dfbc79e3f56f643fd0760cb13d5fb1200f5e600cbc3bb401771b` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | completeDeliveryVerification(DeliveryVerification[1]) seller penalty | 168635 | 816 | 0.00000000013760616 | `0x4f3efc4e50c3f0bf745163cfa8cec766612cc24b13e3c2ae61d7f5d8c783ae92` | Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately. |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 28946 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29152 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 36877 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28936 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28915 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Mainnet/Volta values depend on live gas price at execution time.
