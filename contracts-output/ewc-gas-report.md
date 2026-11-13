# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-15T19:33:13.297Z
- **network**: ewc (246)
- **deployer**: 0x2Cb2Ea6add86E0D53fD8c5B65AF1c4Eca5377275
- **nativeSymbol**: EWT
- **settleBatchSizes**: 1,2,5
- **actorRegistryProxy**: 0x9b98a5c5C3Ad065ed3AFd44150Df2B7CbBf86edF
- **marketControllerProxy**: 0x7a93d486D06327Db2d30dCEfac1dCA9ed6Bcb42d
- **orderRegistryProxy**: 0xF7217b8528E0313aFb8a4e7c85139B296146E88f
- **tradeSettlementProxy**: 0xC31585ADa88aDE2fbA93BD0D4d9345E2C55629Ea

## Totals

- Deployment gas: 7650097
- Deployment fee: 0.00084151067 EWT
- Role setup gas: 317443
- Role setup fee: 0.00003491873 EWT
- Mutating call gas: 7638416
- Mutating call fee: 0.00084022576 EWT
- Deployment + setup + mutating fee: 0.00171665516 EWT

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (EWT) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631291 | 110000000 | 0.00006944201 | `0xa47b1d105eb6afee34d6457a0eeae8b5c05a1e0569883cad3a50ebba18c271fd` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720527 | 110000000 | 0.00007925797 | `0xd80e09e6c2bb300e41c3b5db7c2d31a4c11d69fec71a5f20cda7bd0378ba17d9` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 624949 | 110000000 | 0.00006874439 | `0xd7ed36687d828d532e3b6d0b912a853812058e53b854ba596ffbb8a293a3546f` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 695757 | 110000000 | 0.00007653327 | `0xc81d75fd088bd14bdb459769a9f4c032a464db56c8ab691d36c31368e64fe76b` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1619582 | 110000000 | 0.00017815402 | `0x592f7671d28f83ec654b884f4ad5287c5f53417aff1b7ffb1f935f246f1a66df` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741306 | 110000000 | 0.00008154366 | `0xb6b8cb19f8d8bb715cdb999754f506f27689d18e9112d4f7c91bfbd4295da02e` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1898115 | 110000000 | 0.00020879265 | `0xda54977b3c26afd2833acc6ceef49f2b2134ff7e5cd755484f92418720ded21d` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718570 | 110000000 | 0.0000790427 | `0x8989a11bfbb304dc33945c0517e9a6520f6273822b95d01fce6345a66db7da0c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56591 | 110000000 | 0.00000622501 | `0x17abc6ab4ac10a4f47081a1b208b0852203e4172a2b9ff5c4fc1f5524cc6dd5b` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56658 | 110000000 | 0.00000623238 | `0xd627bd9acc268d00440230f46d32210c877020f861bc091a4a0050679bca5762` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56658 | 110000000 | 0.00000623238 | `0x3a412909b172db73d7054732eef31a4afe10f25b2cc6573b94fc4ac9cd66741a` | Benchmark-only grant used to measure updateStatus directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56591 | 110000000 | 0.00000622501 | `0x3a49ad3c7fd904b9a37521c701b79b01833fd45bc3fa17f467ee7d28dc3bf219` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56591 | 110000000 | 0.00000622501 | `0x07025d851768db491bc6b7c8d0e60730d1264b98b1122478213d4e54d00871b1` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 34354 | 110000000 | 0.00000377894 | `0x9c3018f6e1ac0baa47e9f3db8eb66ae77c09039b27abb3d87f8f0f096ecbff2b` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 110000000 | 0.00000592658 | `0x7a3bdccdfc4c262175c67e55bae834a65bfc8d5b176fb097adefa2e864bbdba5` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 110000000 | 0.00000592658 | `0x017205ca4f1dc2eef7c21a9166ebd9825a2a163c6df43f4825ec7cfe6eef9ded` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 110000000 | 0.00000594891 | `0xc79e6d7a4482c9b6a367b781cbc1b5ded2e64ba791f09c339dd68b70a5fcca3e` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 110000000 | 0.00000353859 | `0xbddf31cdb62f56c111c801dce061769825f7dd2bb45ae113d28e3fb85618c73e` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53570 | 110000000 | 0.0000058927 | `0x080c393519dfabb45f5b9f5b5473227ecdc6255164fc278f9934a48d78ca6c60` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31658 | 110000000 | 0.00000348238 | `0xe14f1a7701604586eb8f176d88ab871c6290e8ab59f835039625ad6a8b11efb2` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 75605 | 110000000 | 0.00000831655 | `0x5f9f17b01f64d6b20d884122291cc67c83219b6c07b65d318fe10a8eb49b3f8d` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31386 | 110000000 | 0.00000345246 | `0x86c916fdba29377022f6309f618328bcfb882684f4f33b9059a42846d9f33fe8` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 58505 | 110000000 | 0.00000643555 | `0x3d388c6b34de76968e1dc5c501f3af5096ed4092f60b5952dbf6567ec7be31c1` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 181790 | 110000000 | 0.0000199969 | `0xfd6a17742b01d425734ef873af362acf9e1670ca57b18ceccc8e9d751502d11e` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 181778 | 110000000 | 0.00001999558 | `0x8894676a00abda760aa64cd796c0bff16e0bc542252504fe43e4fbcc1e913307` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 181790 | 110000000 | 0.0000199969 | `0x613d0fc0ea98763335a1c6128cfbdd420df9882c1141f2f9d66408c62eb19f89` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 72038 | 110000000 | 0.00000792418 | `0x3c7457a3a0321e00a53cffc0e3353735e7e607c408409c2a73660476e07d02bb` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 181790 | 110000000 | 0.0000199969 | `0x10293c066356d6d23973b0f410fcd8dd9b432838c4f3f52515430b8c11814c30` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 36027 | 110000000 | 0.00000396297 | `0xaeef357b899687a8cadab69e46992fe760570efd08db9c7df792df584d1a283e` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 181790 | 110000000 | 0.0000199969 | `0x8d72b3ace1162e0f18eb268a12acc9521026dbceb9b43aa3406f1f0a99c51d7f` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 181778 | 110000000 | 0.00001999558 | `0x0ce914aa771eeca8412ca5cf8644bf02c81bac5d8edef8c3efbdc65aac21bc53` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 181778 | 110000000 | 0.00001999558 | `0x97d955ce8d1eb5e595fa3f8dbad3b75a58c2912556c7b9ce8d031aa0cb26d30c` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 181778 | 110000000 | 0.00001999558 | `0x1d7ab8b13b231d49a6fec5fdc5035fe69cf9374ade81245cd9f290ebea1df2b7` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 181790 | 110000000 | 0.0000199969 | `0xddec0c83bd3f7c2c23cfa228fde3f5af4a9fff0bbb2856a13ced7d8e9e17ee06` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 181778 | 110000000 | 0.00001999558 | `0xf0839a0f09e2159cd6d66791eb008da6ee7fdcb2127e441c4667b0e0cda74e4a` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 181790 | 110000000 | 0.0000199969 | `0x73a1b768f67bc32e48f4f42350968a5905459b4790ba4d8c0b6ec488d75664aa` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 181778 | 110000000 | 0.00001999558 | `0x5e9f834d72e5fd5a6ac71dd915b58042dc9d704ae498c7e586364ad025d387b0` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 181790 | 110000000 | 0.0000199969 | `0x31ae5f3fe5a1ea4542a15ebfd76c440e8cba7f0533894a68bc6842d4946f50b9` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 181766 | 110000000 | 0.00001999426 | `0xf135dc1a5d4103cfbc460ba81f0515dd9f3a897d4a866f23e49a8315bd5397f5` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 181790 | 110000000 | 0.0000199969 | `0x7d6e516df08eedcd18b32f624ef853b5e44b4d731ee4cf8540caab986e9cf443` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 181778 | 110000000 | 0.00001999558 | `0x5f4891705ef2803bff043b8821566c4c22f74acb6c037a0f1c087f638fea1ff3` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 181790 | 110000000 | 0.0000199969 | `0x0780ee542cb5f57c2f4fe1b11a65b900c1312644640d8974e9c6f4908ca67f78` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 181778 | 110000000 | 0.00001999558 | `0x4feff9c297d86f49159c285ef862c874f21a5cede799b1187f9977856f143a41` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 298744 | 110000000 | 0.00003286184 | `0xd3dd263a901b0a84c447816abb18714b4734320555b92a6405d0740e39ecb367` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 538538 | 110000000 | 0.00005923918 | `0x067bc08e5a6117a0df22508d116011234af50f7a1d4576f0b819b1bbc33d46b2` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 1258030 | 110000000 | 0.0001383833 | `0x2dbee7f49a0050bbf5332cc1ff4d7a6fa7a91d5a8de2b66473bb4e15a4b23c4a` | Batch-size benchmark row. Prerequisite order placement gas is reported separately. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 104515 | 110000000 | 0.00001149665 | `0xfa7e3d3b23a997e4b7973aa0dab0862decca2a19713d7d2fb39f295542251358` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification zero penalties bid | 181790 | 110000000 | 0.0000199969 | `0x330ba602815a565526cc8402129dbb68b2cf35276fff66f448f7931c3f6a28ea` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification zero penalties offer | 181778 | 110000000 | 0.00001999558 | `0x7c07974fa3f2c19512311de919e4901ae62b3d81798bdcc940b503668e891cec` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) verification zero penalties | 298744 | 110000000 | 0.00003286184 | `0x2e9b325292846c0cb4fb2c16dc8f3a6144edd06d5880cc34c2f2bdf581613858` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | completeDeliveryVerification(DeliveryVerification[1]) zero penalties | 120435 | 110000000 | 0.00001324785 | `0x7e5aeac4e74c90f6b2e06e3a96e308dacbe53dee3e48da06472aaf529367d0e5` | Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification seller penalty bid | 181790 | 110000000 | 0.0000199969 | `0x07efd8fd5a2bbd318adcc87f7fd9c325b40715593c68b310b13c4fa83cc12eb6` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) verification seller penalty offer | 181778 | 110000000 | 0.00001999558 | `0x7d5160438517c58f208595b382af12734fb93188e195a229b00439d2f0d296be` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) verification seller penalty | 298744 | 110000000 | 0.00003286184 | `0x97111286ed5a16fa16b3db4d91e339c0b34d82a62d70f54b327c36ac7c266ccc` | Prerequisite for delivery verification gas benchmark. |
| Mutating calls | TradeSettlement | completeDeliveryVerification(DeliveryVerification[1]) seller penalty | 168635 | 110000000 | 0.00001854985 | `0x911e909d56ad4d4edc483773f0272dd71553bff739be64a27b819198f9bac811` | Fresh settled trade with no existing trade penalties; prerequisite gas is reported separately. |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29278 |  |  | view estimate |  |
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
