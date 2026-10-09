# GSY DEX Smart Contract Gas Report

## Metadata

- **generatedAt**: 2026-10-05T16:05:20.576Z
- **network**: anvil (31337)
- **deployer**: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
- **nativeSymbol**: ETH
- **settleBatchSizes**: 1,2,5
- **matchTypes**: Standard=0,Preferred=1
- **actorRegistryProxy**: 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512
- **marketControllerProxy**: 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9
- **orderRegistryProxy**: 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707
- **tradeSettlementProxy**: 0xa513E6E4b8f2a923D98304ec87F64353C4D5C853

## Totals

- Deployment gas: 7141253
- Deployment fee: 0.005710552252284788 ETH
- Role setup gas: 339814
- Role setup fee: 0.000116351647287003 ETH
- Mutating call gas: 18280074
- Mutating call fee: 0.000127642644471532 ETH
- Deployment + setup + mutating fee: 0.005954546544043323 ETH

## Detailed Values

| Section | Contract | Action | Gas | Gas Price (wei) | Fee (ETH) | Tx | Notes |
|---|---|---|---:|---:|---:|---|---|
| Deployment | ActorRegistry | ActorRegistry implementation deployment | 631465 | 1265625000 | 0.000799197890625 | `0xfc76181135dff9bc0dfe5846f26b9569c1231df288b7476849c88c6cbf9aa74c` |  |
| Deployment | ActorRegistry | ActorRegistry proxy + ProxyAdmin deployment and initialization | 720849 | 1114081858 | 0.000803084793257442 | `0x9c31c55da1c2c2cd1de89546ddd1d5a8a58582874e1042e3062b2f3c421d6d1f` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | MarketController | MarketController implementation deployment | 584915 | 981513999 | 0.000574102260725085 | `0xbe812ee0fd9c0d4800d0ca1d71dc09c2e692d638b0058641f12521c6812cf252` |  |
| Deployment | MarketController | MarketController proxy + ProxyAdmin deployment and initialization | 696079 | 863608934 | 0.000601140043169786 | `0x8064f638f0e9d85a2602882da093409a517ba4c205860e0cf773b256c09b34b7` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | OrderRegistry | OrderRegistry implementation deployment | 1450498 | 760667319 | 0.001103346424874862 | `0xe5ee02bdffad6ffda1bf549bf9900dc8285006675215b277d582970d757e82cf` |  |
| Deployment | OrderRegistry | OrderRegistry proxy + ProxyAdmin deployment and initialization | 741598 | 674778458 | 0.000500414354895884 | `0xe76fde5bedddeb5b746a090e2966cd1cf696e338a1928f1dd2090930c1b262b3` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Deployment | TradeSettlement | TradeSettlement implementation deployment | 1596999 | 594601271 | 0.000949577635185729 | `0x6337f16aa00a376b9f008a0d73fa1281c1d1785e397e0bd9107f27cda4ab6440` |  |
| Deployment | TradeSettlement | TradeSettlement proxy + ProxyAdmin deployment and initialization | 718850 | 528189260 | 0.000379688849551 | `0xff9e67fec6c1b72862c28d8b7e005dc45e67315d76887722d29306684a99ac7c` | Includes TransparentUpgradeableProxy deployment, ProxyAdmin deployment, and initializer delegatecall. |
| Role setup | MarketController | grantRole(ORCHESTRATOR_ROLE) | 56635 | 465329677 | 0.000026353946256895 | `0xcd7a658b804fd87354daf198f9570ab13613d2b6a18604dde7627c3c6d9a15cf` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, TradeSettlement) | 56658 | 407383084 | 0.000023081510773272 | `0x4d81779fa99d44301fd00acfe893e192681ff7b927b1ddf5fc99210b1f655bf6` |  |
| Role setup | OrderRegistry | grantRole(SETTLEMENT_ROLE, benchmark signer) | 56658 | 356652545 | 0.00002020721989461 | `0x4bc74073f8443f3e40a2827945656002d064c78b36b3d2a9ed432cda84df6589` | Benchmark-only grant used to measure updateStatus and settleOrder directly. |
| Role setup | TradeSettlement | grantRole(OPERATOR_ROLE) | 56636 | 312239370 | 0.00001768398895932 | `0x4f5edb967ef1901b951a3b056049d8e25783f28af51704477b38a5d852c7c3f6` |  |
| Role setup | TradeSettlement | grantRole(EXECUTION_ENGINE_ROLE) | 56636 | 273356816 | 0.000015481836630976 | `0x0dd9cd4afcf06fab9837152cdaf557eb42a86af94a5b36005cbfc520c04710bf` |  |
| Role setup | ActorRegistry | grantRole(ACTOR_REGISTRAR_ROLE) | 56591 | 239316230 | 0.00001354314477193 | `0x1112d078ded0a4a2e3f82f3a6a2d56d1931eca2730e0babc844535e3d6e532f1` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) | 53878 | 209514562 | 0.000011288225571436 | `0x41eedd06890868de494295329de688cc1fb19c3bfbc22bd6f5c159c060503d66` |  |
| Mutating calls | ActorRegistry | registerActor(bytes16,address) second actor | 53878 | 183419310 | 0.00000988226558418 | `0x665a375271404161583e1112ce6df7fe317808f89fef1627ebebcca4579c20a4` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,true) | 54081 | 160574249 | 0.000008684015960169 | `0x196128372273cf8df2a1b8e2efd20006e03dca9bd4562d3934a3a585f15628d4` |  |
| Mutating calls | ActorRegistry | setActorWallet(bytes16,address,false) | 32169 | 140574835 | 0.000004522151867115 | `0x541d5feb8fefafbf15734b0e93ae63c62983bba7bf361789938b0d0ce12c7c4e` |  |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,true) | 53774 | 123040666 | 0.000006616388773484 | `0x94ee6beeb9b355d140f82a640871372698e7cc97b2eb0f9df083e8135c71a479` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | ActorRegistry | setProxy(bytes16,address,false) | 31862 | 107715721 | 0.000003432038302502 | `0xbccc72b91f418b5e57428ecbafb81e280fdb4ae9012de97e6600c070fcedbc0f` | Uses a benchmark delegate distinct from the order signer. |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) | 53056 | 94279857 | 0.000005002112092992 | `0x6841a85e1b8611f4d4792b940f0b0bd6389579c1457a694739efb328129605e2` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,false) | 31144 | 82536559 | 0.000002570518593496 | `0xd114f782558cda783cc8628a710f12c1f4a7a86d959352bfa58c8dafee7c6eac` |  |
| Mutating calls | MarketController | setMarketStatus(bytes16,true) reopen | 53056 | 72240911 | 0.000003832813774016 | `0x9d2854c0c8f036c586a4b3668472484903a17355aca847d866c3ddb452678e85` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) bid | 156055 | 63242738 | 0.00000986934547859 | `0x192736c67c328db1c72b4a3c53fcd4e5e917abba64bf260da89432d99812c7c6` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) offer | 156043 | 55419640 | 0.00000864784688452 | `0x858b83c6d5ac17a63809852e9eb00e4e5aea0e3ff0d4b1c0e5e427ca1db05f58` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) cancellable order | 156055 | 48564251 | 0.000007578694189805 | `0x01ed4544a3ce71f131f6b4a38d0bc4053d638e4278094643799d6be20e9d28cf` |  |
| Mutating calls | OrderRegistry | cancelOrder(OrderParams) | 51378 | 42556876 | 0.000002186487175128 | `0x44514b7188b2268db6b16833d43d948b7b12a9218b9eff286fb6ac58c330ee68` |  |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) status benchmark order | 156055 | 37255488 | 0.00000581390517984 | `0x8109731cda3958e7a3f28f2d9b042ea57b3ce2af81f4b996063f243fff8f23b5` |  |
| Mutating calls | OrderRegistry | updateStatus(bytes16,OrderStatus) | 35972 | 32647001 | 0.000001174377919972 | `0xf08d4fcb929330362060233f42f58fc06e857a8e6f68891efd5917bce16ef171` | Measured with benchmark-only settlement role granted to deployer. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleOrder benchmark | 156055 | 28575914 | 0.00000445941425927 | `0x720ae1ef2037eb8119e13a21f5a883096d1aa4e58a00b74e647a06563d2b0c6e` |  |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) partial fill | 145074 | 25041086 | 0.000003632810510364 | `0x0b6aaa22e332d34f3930f90c65a72c8463daa7f13f50e4716d1dd44803b6398e` | Benchmark-only direct call; consumes parent and registers a 50000 residual bid with inherited metadata. |
| Mutating calls | OrderRegistry | settleOrder(bytes16,uint64,bytes16) consume residual | 45853 | 21941224 | 0.000001006070944072 | `0x4be5d9af61b5943625dcdd6443ac6200e002f763612a476684a0f46f1aa79511` | Benchmark-only direct call; fully consumes the previously registered residual bid. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 1 | 156055 | 19206955 | 0.000002997341362525 | `0xe1f5b1a636c5797df4c4c33089c948c17b1a4063f649a0257d381ec7ba1ea26d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 1 | 156043 | 16831064 | 0.000002626369719752 | `0xe5b44b375837fd9925120229b000af0d0b0c74b8058950a2c1151feb6eebe125` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] bid 2 | 156043 | 14749068 | 0.000002301488817924 | `0xf47cfaac7f3723d6db2e9bdcfa69df8ad3ba96d6314a9a35b06984cbba753f62` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[2] offer 2 | 156043 | 12924614 | 0.000002016795542402 | `0xdbfdfe37f5841502da7d04880a3d6110b6e5e76cc96f2a05dcddb399c2eb9c37` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 1 | 156055 | 11325845 | 0.000001767454741475 | `0xb7e16a740bc3a6c2dda48939be487c334884d1f870a8cfc81679192dd05ce130` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 1 | 156043 | 9924843 | 0.000001548702276249 | `0xd294e6a312541a09c7d684fb43773b1209bd404aa2efe518d1ca03c8f8dca000` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 2 | 156055 | 8697144 | 0.00000135723280692 | `0xd7b04e902e9a7786914315ee6ba5298684267c78397f269d230cc4f75404d721` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 2 | 156043 | 7621312 | 0.000001189252388416 | `0x6ca3e0e596f69ab16584794ad1f076e32f69d90f6b545bd0e9c339345e73f42d` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 3 | 156055 | 6678560 | 0.0000010422226808 | `0x9c2de5ac93e47533d75655751026b67c16e03ef62e2ea6d27e38c1b1c5898519` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 3 | 156031 | 5852425 | 0.000000913159725175 | `0x2ad437e2c50ee1d39751ea2f824710c6c9abfabecd0b77a947eab510fd36b483` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 4 | 156055 | 5128483 | 0.000000800325414565 | `0x47162939647389a405f3eb2d9aaf78345672f3f148e73c4f0e35702de2d6d81d` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 4 | 156043 | 4494092 | 0.000000701271597956 | `0x796af65eb0752147e247722dea5f7fa139ead7263c6cfea3b314934c299e49b3` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] bid 5 | 156055 | 3938175 | 0.000000614571899625 | `0x54d54ebb17606aa81b91805823fb376eeb479f203db1b7bb69183e4491d080ed` | Prepares unique open bid for settleBatch gas benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) settleBatch[5] offer 5 | 156043 | 3451025 | 0.000000538508294075 | `0x857a2dbeee8655cef03f287cd6b2060259411f5f5672a7112d808c7377012345` | Prepares unique open offer for settleBatch gas benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) | 217738 | 3024135 | 0.00000065846910663 | `0x0a17a5f17a3d00b16d07ac2493385c135ee0910bc6581dfac59f977dca16f717` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) | 389527 | 2651605 | 0.000001032871740835 | `0xb9a5a023200250489926f5143d36c90b6a3035121b81a1df80db0549eb301fef` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) | 904997 | 2328762 | 0.000002107522623714 | `0xb6f4ee228c7a3abfdcc7e798e06e60295d90b8c8210c3c1eb7121051bc6bee02` | Standard matchType=0; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] bid 1 | 176159 | 2055230 | 0.00000036204726157 | `0x56f3201e249c1a5f68f5d4a93c2e7b3121ca4cd75e3573ef7c0cab875a9a4dd5` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [1] offer 1 | 156043 | 1801345 | 0.000000281087277835 | `0xa96840f288104958d93539d740c2fba8d4b04bf3879b0728bff0a5a4ed3c4212` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred buyer-only | 219163 | 1578520 | 0.00000034595317876 | `0x49fa945a6c919bc748c5301afd3adc7bc65f1e141f62cd48e9507ff884bc01ce` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 1 | 176171 | 1384088 | 0.000000243836167048 | `0xc212cd3872bbc56b62758a23b3ec3ea59263cbe8340678df5b0e955d8fe334a1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 1 | 156031 | 1213110 | 0.00000018928276641 | `0x8fd3ea86832864b4c857cbc2623080df7952977ef560c0516d503d06fd5e2bd1` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] bid 2 | 176171 | 1063049 | 0.000000187278405379 | `0x7f1baee079807b77907b9a96c8facc80f02bb8838549f682a69d9ca60b8a89d0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [2] offer 2 | 156043 | 931729 | 0.000000145389788347 | `0x944d8c150e93e6bc18bb354502c6f9c436fac99f734e64c36441908aa68dd635` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred buyer-only | 392401 | 816474 | 0.000000320385214074 | `0x5c1851605f2ce807c4d5315ab0ed08d8e4900a79dbdb7925a0781ace9e9d9ea6` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 1 | 176159 | 717086 | 0.000000126321152674 | `0x3b46d69ab4cec91c4aa7139d5598298d408b89d8e9d0f750084358a2271ec86c` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 1 | 156043 | 628504 | 0.000000098073649672 | `0x2c505c029eef725e06f2d0df3ab93faaa7ea09673fd38435831583341d7732dc` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 2 | 176159 | 550759 | 0.000000097021154681 | `0x20a929f91d1e2ef289b9fbdb0bdb0f6914f700d2e869c183d5314b15282c278a` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 2 | 156043 | 482723 | 0.000000075325545089 | `0x99cff7651a657f187071cc6a374029596c55e4117c4321ea6133b5085ba80dae` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 3 | 176171 | 423011 | 0.000000074522270881 | `0xa15cd2975b1abe2f708423d0b403fdb2c4b817fae0ba40df124f061f8304feb8` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 3 | 156043 | 370755 | 0.000000057853722465 | `0xebfc1fbdfbfcd55f6f21db25fdcc27ae373e6f9b5f90fc3e0f4f2c26788b9c44` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 4 | 176171 | 324894 | 0.000000057236900874 | `0x797aa2c76db66da90ce5ea67409e8e1aff87b0b7759791ad2f98ac6fce1ba1c8` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 4 | 156043 | 284760 | 0.00000004443480468 | `0x87926e6d977cf36153914d5988b62578295c1cadc6c45c8c0a16d6031c53a6cb` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] bid 5 | 176171 | 249536 | 0.000000043961006656 | `0xfaae08612163c467a84a9c8638059b3ecaeb9c10f53b83aacc2f224ad990f461` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred buyer-only [5] offer 5 | 156043 | 218711 | 0.000000034128320573 | `0x3aa6cbaa502439914f69f4cf5d335b8a4031ae758df3d077cf35a06d424332ce` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred buyer-only | 912170 | 191657 | 0.00000017482376569 | `0x05cf33944413ca68b6c5c79602d93d4e2fd37db62fec83fb61f06076c92330cd` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] bid 1 | 156055 | 169157 | 0.000000026397795635 | `0xac7d71e9bab078b8c406ad439e3c015026113b8c59946201384f78ba6a1938fa` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [1] offer 1 | 176159 | 148232 | 0.000000026112400888 | `0x07411734800bf51981039dc2a7b41b668d9748cf572686389c255d3d026d3eeb` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred seller-only | 239104 | 129921 | 0.000000031064630784 | `0x9c2ff1da107ab471de8d2c7b0fbe6e5fccba7bd555e65f94e8eeed0bed005cba` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 1 | 156055 | 113940 | 0.0000000177809067 | `0x99a2ac419c3b864a39026b760177a1581673f2abfcd49b2989060b9c4e3151df` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 1 | 176159 | 99846 | 0.000000017588771514 | `0x9473df2e381e5fd16cd0335f96ebe8da9228231f973d32175b405ef52260fc14` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] bid 2 | 156055 | 87512 | 0.00000001365668516 | `0xea5feb1fca9dd13c424c89d68b0c76ed9b4f70779f8d7c8a7991981789617b2f` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [2] offer 2 | 176159 | 76688 | 0.000000013509281392 | `0x6f5031b3d1265e7a4c73601f5ff15b92db6a3d6d73ed59e2c017bea79f7b6214` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred seller-only | 432271 | 67214 | 0.000000029054662994 | `0x2120cc9dc59cc66132aa4cfd0dec49dfdf67867c7feec432e31a626f94d09404` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 1 | 156055 | 59055 | 0.000000009215828025 | `0xd3f02b6ea31fe64b7efda70ba5237cee1a9dd0de10c55d01e5d5c7299583cb4f` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 1 | 176159 | 51751 | 0.000000009116404409 | `0xeba7f6011712fbba4e06ef07ae15f422fcba53078f78f25c2b0bbfc0b45d2f66` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 2 | 156055 | 45358 | 0.00000000707834269 | `0xf522e3f3c37d4fe81244a245eec629418409b84b9dd9ab84f45349571e2379af` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 2 | 176147 | 39748 | 0.000000007001490956 | `0xa742c4d4f3eaa88176c753482fe616307d0a21c7590bfbc0b2591638f68655ff` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 3 | 156055 | 34838 | 0.00000000543664409 | `0x0c1221f251b03cfbefdaaf96329b46f7ae43235ae07f2f596e6b38b20b4048a4` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 3 | 176159 | 30529 | 0.000000005377958111 | `0x70134a9c670a41277238df2c69b210b340bc4afe010be57fddf31bf7e4bbd5d0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 4 | 156055 | 26759 | 0.000000004175875745 | `0x9c30a1c29bc862d2866c47f815187a1e5f02e820e35d66aed801689ed94299e8` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 4 | 176159 | 23449 | 0.000000004130752391 | `0x4e7cfcdfd85b099ebef0907be8ed0ccc9af78818501da9e2b635ae577779801b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] bid 5 | 156055 | 20553 | 0.000000003207398415 | `0x43d51b6cf03e20275dc1e0d4af3c1be3287a4f9b591b015bb1bc652048db8093` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred seller-only [5] offer 5 | 176159 | 18012 | 0.000000003172975908 | `0x8f6086098882af249dfddf1c13fedabe93b7689b7c708a04a5f139eaee31f338` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred seller-only | 1011839 | 15787 | 0.000000015973902293 | `0x0ef2294cc0216bb3bf1ea36952c50940d7f070fb0bd3469980bcb124dae30380` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] bid 1 | 176171 | 13947 | 0.000000002457056937 | `0x0c961c2f1f71dd735d5d8121714f8e88c8b7bb36da6adaf723b19af6b863fc5f` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [1] offer 1 | 176159 | 12224 | 0.000000002153367616 | `0x1ff0844416a621a509e6b6f724557f2fc0af13ea05a959d4e7e459b49c8584ca` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[1]) preferred reciprocal | 239511 | 10714 | 0.000000002566120854 | `0x890e815b3f1a4c77a74fd4f144d1dbf5fd026deca245f8073ae7e1405110db6c` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 1 | 176171 | 9397 | 0.000000001655478887 | `0xdf43b3064c4aa28647d1547d8c4a25827947eef6a3f7b82701e6fd86cb4994fa` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 1 | 176159 | 8236 | 0.000000001450845524 | `0x76b788da600d8e8787f64478184a7681a9227bc337aa75b5a45a20e7fae1b719` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] bid 2 | 176159 | 7220 | 0.00000000127186798 | `0xa84eba264989a426a3e1bfe1ac596d70b9a7deb9fecc8225098a63eb63fc4d53` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [2] offer 2 | 176159 | 6329 | 0.000000001114910311 | `0xeeb85761b212c0e285e8fb69adf9801838beae5ca61d43d8f128964de939250c` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[2]) preferred reciprocal | 433073 | 5548 | 0.000000002402689004 | `0xdc77d50d9bc4593daf12fa4702149973d35687ddd6737d41ab25f2583595f03e` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 1 | 176171 | 4875 | 0.000000000858833625 | `0xa1d02b01c8251935b59d1ccc5a89b9e9b238d1fbd08ef93ad28298c744bb3569` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 1 | 176159 | 4274 | 0.000000000752903566 | `0xc68248bfc494d11ad3f8710c44c24a049c193c04b25a5b365c48cdc2df2e0d84` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 2 | 176171 | 3746 | 0.000000000659936566 | `0xc74ded04f7ed31fadc41befdf6551a6bb22f1ac0a438b6b2de39196708607597` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 2 | 176159 | 3284 | 0.000000000578506156 | `0x4540c82acccf9ebdd00813d1f33a7f01e5c6ef092eb9a4980c457dad875f9cd0` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 3 | 176159 | 2879 | 0.000000000507161761 | `0xa5353991c03965e2e3ea0ed8dc6639c0601a6790ab70826e10dac917adeca74b` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 3 | 176159 | 2523 | 0.000000000444449157 | `0xe2b7e025ad6980b65f7407bdaa89188a3ec565c2f0128963ddfec496ccbfee62` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 4 | 176171 | 2212 | 0.000000000389690252 | `0x997ac7947f44310a969fa0928a3925d83d566d665c1cd4fadc1cb2c909eb9605` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 4 | 176159 | 1940 | 0.00000000034174846 | `0x6406f3d2f04867ce25f844acfedb3d806c8e80ee94a36e578c1231f30fd41ffb` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] bid 5 | 176159 | 1701 | 0.000000000299646459 | `0x2e2df129c16a281d57e7cfa6aa366fcb63cee798638b2a47aff9b8364d768d34` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | OrderRegistry | placeOrder(OrderParams) preferred reciprocal [5] offer 5 | 176159 | 1490 | 0.00000000026247691 | `0x536059129d521da2e7f74607b4dc5ea2c39daa921d629316a8e5d042ebc200ee` | Prepares a unique open order for the preferred settlement benchmark. |
| Mutating calls | TradeSettlement | settleBatch(Match[5]) preferred reciprocal | 1013850 | 1307 | 0.00000000132510195 | `0x23f5fbb453441a259d77ae2767dcc669d969f840b5b39b22bf7e3bad2c71fff0` | Preferred matchType=1; includes residual offer registration for each match. Order placement gas is separate. |
| Mutating calls | TradeSettlement | submitPenalties(TradePenalty[1]) | 80362 | 1155 | 0.00000000009281811 | `0xf9b6274e7e5c6e51c4c4bbf2d50b1fe9d51c9dbe365956d74e79b5b288a327e7` |  |
| View estimates | ActorRegistry | isAuthorized(bytes16,address) | 29493 |  |  | view estimate |  |
| View estimates | ActorRegistry | isProxy(bytes16,address) | 29482 |  |  | view estimate |  |
| View estimates | MarketController | isMarketOpen(bytes16) | 28991 |  |  | view estimate |  |
| View estimates | OrderRegistry | getStatus(bytes16) | 29062 |  |  | view estimate |  |
| View estimates | OrderRegistry | getOrder(bytes16) | 36899 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByTrade(bytes16) | 28892 |  |  | view estimate |  |
| View estimates | TradeSettlement | penaltyEnergyByActor(bytes16) | 28871 |  |  | view estimate |  |

## Notes

- View functions are reported as `estimateGas` values only; they do not consume gas when called off-chain.
- Proxy deployment rows include the `TransparentUpgradeableProxy`, the internally created `ProxyAdmin`, and initializer delegatecall gas.
- `settleBatch(Match[N])` rows are measured for `GAS_REPORT_SETTLE_BATCH_SIZES` values; prerequisite dummy order placements are reported as separate mutating calls.
- Standard (matchType=0) and preferred (matchType=1: buyer-only, seller-only, reciprocal) batches each fill 100000 of a 100000 bid and a 150000 offer per match, at price 12000. Each match includes on-chain creation of a 50000 residual offer.
- Preferred benchmarks set both effective rates to 12000. The counterpart without a preferred partner uses its normal rate when no preferred rate is supplied. Differences include calldata and storage access costs, not just validation instructions.
- Direct `settleOrder` rows use a benchmark-only settlement role. Production `settleBatch` rows already include these internal calls; do not add their gas again when estimating a trade.
- Totals include all benchmark fixtures and alternative cases, not the cost of a single production deployment/trading workflow. Local fees are illustrative, not remote-network price estimates.
- Mainnet/Volta values depend on live gas price at execution time.
